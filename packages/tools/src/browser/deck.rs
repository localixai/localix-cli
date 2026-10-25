//! The one window: the browser's tabs and the terminal sessions' screens are tabs of it, with a
//! strip along its top (strip.rs) that names them and switches between them. The browser's tabs
//! are the agent's (tabs.rs): showing a terminal moves them aside, still rendering, and never
//! changes the tab the agent acts on. A shown window opens with the first of either; a headless
//! one is the browser's alone, with no strip. It closes when nothing is left in it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tao::dpi::LogicalSize;
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use wry::{NewWindowResponse, WebView, WebViewBuilder};

use super::frame::{self, Frame, Look};
use super::host::{Cmd, Shared, Window};
use super::panel::{self, Open, Opening};
use super::strip::{self, HEIGHT};

#[path = "deck_acts.rs"]
mod acts;
#[path = "deck_layout.rs"]
mod layout;
use super::tab_view::build;
use super::tabs::Tabs;
use super::viewport::Viewport;
pub(super) use acts::After;

/// What fills the window under the strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shown {
    /// The browser's active tab.
    Web,
    /// A terminal session's screen: its panel's id.
    Term(u32),
}

pub(super) struct Deck {
    pub frame: Frame,
    look: Look,
    /// The strip, in a shown window.
    strip: Option<WebView>,
    pub web: Option<Tabs>,
    terms: Vec<Open>,
    pub shown: Shown,
    /// The history menu is open: it takes the window, the pages stand aside.
    menu: bool,
    /// The strip has its row of projects: the window holds the tabs of more than one.
    rows: AtomicBool,
}

impl Deck {
    /// Whether the window is on the person's screen (it has its strip), not headless.
    pub(super) fn is_shown(&self) -> bool {
        self.strip.is_some()
    }

    /// The window for the browser, on its first tab.
    pub(super) fn for_web(
        target: &EventLoopWindowTarget<Cmd>,
        window: &Window,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        let mut deck = Self::build(
            target,
            &window.title,
            &window.viewport,
            window.headless,
            proxy,
        )?;
        deck.open_web(window, shared, proxy)?;
        Ok(deck)
    }

    /// The window for a terminal session's screen, before the browser opened.
    pub(super) fn for_term(
        target: &EventLoopWindowTarget<Cmd>,
        opening: Opening,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        let mut deck = Self::build(target, "llx", &Viewport::DESKTOP, false, proxy)?;
        deck.add_term(opening, proxy)?;
        Ok(deck)
    }

    /// The window with nothing in it yet but its strip (when shown), its content `viewport`.
    fn build(
        target: &EventLoopWindowTarget<Cmd>,
        title: &str,
        viewport: &Viewport,
        headless: bool,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<Self, String> {
        let look = frame::look(headless, frame::glass_allowed());
        let top = if headless { 0.0 } else { HEIGHT };
        // A shown window fits the person's screen; a hidden one keeps the page's own size.
        let screen = (target.primary_monitor()).map(|monitor| {
            let size = monitor.size().to_logical::<f64>(monitor.scale_factor());
            frame::moderate((size.width, size.height))
        });
        let (width, height) = match (headless, screen) {
            (false, Some(fit)) => fit,
            _ => (viewport.width, viewport.height + top),
        };
        let size = LogicalSize::new(width, height);
        // Coated in the system's appearance: the titlebar and the strip are not the bare
        // material's grey. The browser's pages paint their own ground over it, opaque.
        let mut frame = Frame::build(target, (title, size, None), look, None)?;
        let strip = match headless {
            true => None,
            false => {
                let strip = strip_view(&frame.native, look, proxy.clone())?;
                // Shows the window too, without making it the key window.
                frame.show(&strip);
                Some(strip)
            }
        };
        Ok(Self {
            frame,
            look,
            strip,
            web: None,
            terms: vec![],
            shown: Shown::Web,
            menu: false,
            rows: AtomicBool::new(false),
        })
    }

    /// Open the browser in the window, and show it.
    pub(super) fn open_web(
        &mut self,
        window: &Window,
        shared: &Arc<Shared>,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<(), String> {
        let tabs = Tabs::open(&self.frame.native, window, shared, proxy)?;
        if self.strip.is_none() {
            // A headless window shows with its first page.
            self.frame.show(tabs.active());
        }
        self.web = Some(tabs);
        self.shown = Shown::Web;
        Ok(())
    }

    /// Add a terminal session's screen as a tab, and show it.
    pub(super) fn add_term(
        &mut self,
        opening: Opening,
        proxy: &EventLoopProxy<Cmd>,
    ) -> Result<(), String> {
        if let Some(ground) = opening.3 {
            self.frame.ground(ground);
        }
        let open = panel::open(opening, &self.frame.native, self.look, proxy.clone())?;
        self.shown = Shown::Term(open.id);
        self.terms.push(open);
        Ok(())
    }

    pub(super) fn term(&self, id: u32) -> Option<&Open> {
        self.terms.iter().find(|open| open.id == id)
    }

    /// Take terminal `id` out, telling its caller; whether anything is left in the window.
    pub(super) fn remove_term(&mut self, id: u32) -> bool {
        if let Some(at) = self.terms.iter().position(|open| open.id == id) {
            let gone = self.terms.remove(at);
            gone.slot.alive.store(false, Ordering::SeqCst);
        }
        self.settle();
        self.web.is_some() || !self.terms.is_empty()
    }

    /// The browser is gone from the window (closed by the agent, or its last tab by the person);
    /// whether anything is left in it.
    pub(super) fn remove_web(&mut self) -> bool {
        self.web = None;
        self.settle();
        !self.terms.is_empty()
    }

    /// Every terminal's caller is told the window is gone.
    pub(super) fn let_go(&self) {
        for open in &self.terms {
            open.slot.alive.store(false, Ordering::SeqCst);
        }
    }

    /// Show `shown`, if it is there.
    pub(super) fn show(&mut self, shown: Shown) {
        let there = match shown {
            Shown::Web => self.web.is_some(),
            Shown::Term(id) => self.term(id).is_some(),
        };
        if there {
            self.shown = shown;
        }
    }

    pub(super) fn titled(&mut self, id: u32, title: String) {
        if let Some(open) = self.terms.iter_mut().find(|open| open.id == id) {
            open.title = title;
        }
    }

    /// What is shown is still there: else the newest terminal, else the browser.
    fn settle(&mut self) {
        let gone = match self.shown {
            Shown::Web => self.web.is_none(),
            Shown::Term(id) => self.term(id).is_none(),
        };
        if gone {
            self.shown = match self.terms.last() {
                Some(open) => Shown::Term(open.id),
                None => Shown::Web,
            };
        }
    }
}

/// The strip's page, which says what the person clicked and nothing else.
fn strip_view(
    native: &tao::window::Window,
    look: Look,
    proxy: EventLoopProxy<Cmd>,
) -> Result<WebView, String> {
    let first = AtomicBool::new(true);
    let loaded = proxy.clone();
    let builder = WebViewBuilder::new()
        .with_html(frame::dress(strip::PAGE, look))
        .with_transparent(look == Look::Glass)
        .with_incognito(true)
        .with_focused(false)
        .with_background_throttling(wry::BackgroundThrottlingPolicy::Disabled)
        .with_navigation_handler(move |_| first.swap(false, Ordering::SeqCst))
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_ipc_handler(move |request| {
            let _ = proxy.send_event(Cmd::Clicked(request.body().clone()));
        })
        // Its tabs are drawn once its page can draw them.
        .with_on_page_load_handler(move |_, _| {
            let _ = loaded.send_event(Cmd::Strip);
        });
    build(builder, native).map_err(|error| error.to_string())
}
