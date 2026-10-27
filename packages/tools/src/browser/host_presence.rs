//! Applies the window's presence (presence.rs, ARCHITECTURE.md A9) on the event loop: the deck
//! concealed or revealed, the mini built, shown and fed (its picture, its tab, its spinner and
//! blink), and where the person left each remembered. The agent's acts reach it as events, and
//! nothing here expands the window but the person's click.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use tao::event::WindowEvent;
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::window::WindowId;
use wry::WebView;

use super::super::capture::Shot;
use super::super::deck::{Deck, Shown};
use super::super::mini::Mini;
use super::super::placement::{self, Rect};
use super::super::platform;
use super::super::presence::{Effect, Event, Presence, State, Support, Timing};
use super::{Cmd, Shared};

/// How long a picture may take before the mini goes on without it.
const PICTURE: Duration = Duration::from_secs(12);

/// What an effect is applied with.
pub(super) struct Stage<'a> {
    pub deck: &'a mut Option<Deck>,
    pub target: &'a EventLoopWindowTarget<Cmd>,
    pub shared: &'a Arc<Shared>,
    pub proxy: &'a EventLoopProxy<Cmd>,
}

pub(super) struct Presenter {
    presence: Presence,
    mini: Option<Mini>,
    /// The tab the mini names now.
    named: Option<(Shown, u32)>,
}

impl Presenter {
    pub(super) fn new() -> Self {
        let support = if cfg!(target_os = "macos") {
            Support::Mini
        } else {
            Support::WindowOnly
        };
        Self {
            presence: Presence::new(support, Timing::default()),
            mini: None,
            named: None,
        }
    }

    /// The state as a check reads it.
    pub(super) fn name(&self) -> &'static str {
        match self.presence.state() {
            State::Hidden => "hidden",
            State::Mini => "mini",
            State::Expanded => "expanded",
        }
    }

    pub(super) fn event(&mut self, event: Event, stage: &mut Stage<'_>) {
        for effect in self.presence.event(event) {
            self.apply(effect, stage);
        }
    }

    /// After every turn of the loop: a window that came or went, a tab that changed.
    pub(super) fn sync(&mut self, stage: &mut Stage<'_>) {
        let shown = stage.deck.as_ref().is_some_and(Deck::is_shown);
        match (shown, self.presence.state()) {
            (true, State::Hidden) => self.event(Event::Appeared, stage),
            (false, State::Mini | State::Expanded) => self.event(Event::Vanished, stage),
            (true, State::Mini | State::Expanded) | (false, State::Hidden) => {}
        }
        let now = stage.deck.as_ref().map(Deck::signature);
        if self.presence.state() == State::Mini && now != self.named {
            self.named = now;
            self.name_tab(stage);
            self.event(Event::Changed, stage);
        }
    }

    fn apply(&mut self, effect: Effect, stage: &mut Stage<'_>) {
        match effect {
            Effect::Enter(state) => self.enter(state, stage),
            Effect::Capture => Self::capture(stage),
            Effect::Busy(on) => {
                if let Some(mini) = &self.mini {
                    mini.busy(on);
                }
            }
            Effect::Blink(long) => {
                if let Some(mini) = &self.mini {
                    mini.blink(long);
                }
            }
        }
    }

    fn enter(&mut self, state: State, stage: &mut Stage<'_>) {
        match state {
            State::Hidden => self.hide(),
            State::Mini => {
                if let Some(deck) = stage.deck.as_ref() {
                    deck.frame.conceal();
                }
                if self.mini.is_none() {
                    self.mini = Self::build(stage);
                }
                self.named = stage.deck.as_ref().map(Deck::signature);
                self.name_tab(stage);
                if let Some(mini) = &self.mini {
                    mini.show();
                }
            }
            State::Expanded => {
                self.hide();
                if let Some(deck) = stage.deck.as_ref() {
                    deck.frame.reveal();
                }
            }
        }
    }

    fn hide(&self) {
        if let Some(mini) = &self.mini {
            mini.hide();
        }
    }

    /// The mini, where the person left it, else in the corner.
    fn build(stage: &Stage<'_>) -> Option<Mini> {
        let area = platform::work_area(stage.target)?;
        let kept = crate::lock(&stage.shared.kept).mini;
        let at = placement::mini(area, kept);
        match Mini::new(stage.target, stage.proxy, at) {
            Ok(mini) => Some(mini),
            Err(error) => {
                super::super::debug(|| format!("the mini could not open: {error}"));
                None
            }
        }
    }

    fn name_tab(&self, stage: &Stage<'_>) {
        let tab = stage
            .deck
            .as_ref()
            .and_then(|deck| deck.shown_tab(stage.shared));
        if let (Some(mini), Some((id, group))) = (&self.mini, tab) {
            mini.tab(&id, &group);
        }
    }

    /// The tab's name again, once its project is known.
    pub(super) fn rename(&self, stage: &Stage<'_>) {
        if self.presence.state() == State::Mini {
            self.name_tab(stage);
        }
    }

    /// Ask for the shown tab's picture; it comes back as [`Cmd::Pictured`], taken on a thread of
    /// its own so the loop never waits on the compositor.
    fn capture(stage: &Stage<'_>) {
        let (reply, shot) = mpsc::sync_channel(1);
        match stage.deck.as_ref() {
            Some(deck) => deck.capture_shown(reply),
            None => drop(reply.send(Err("no window".into()))),
        }
        let proxy = stage.proxy.clone();
        let wait = move || {
            let shot = shot
                .recv_timeout(PICTURE)
                .unwrap_or_else(|error| Err(error.to_string()));
            let _ = proxy.send_event(Cmd::Pictured(shot));
        };
        // Ends within `PICTURE`: nothing has to stop it.
        let thread = std::thread::Builder::new().name("llx-preview".into());
        if let Err(error) = thread.spawn(wait) {
            super::super::debug(|| format!("no preview: {error}"));
            let _ = stage
                .proxy
                .send_event(Cmd::Pictured(Err(error.to_string())));
        }
    }

    /// A picture came (or failed): the mini shows it, and the next one is taken if asked for.
    pub(super) fn pictured(&mut self, shot: &Result<Shot, String>, stage: &mut Stage<'_>) {
        if let (Some(mini), Ok(shot)) = (&self.mini, shot) {
            mini.picture(shot);
        }
        self.event(Event::Pictured, stage);
    }

    pub(super) fn owns(&self, id: WindowId) -> bool {
        self.mini.as_ref().is_some_and(|mini| mini.id() == id)
    }

    /// The mini's page, for checks.
    pub(super) fn page(&self) -> Option<&WebView> {
        self.mini.as_ref().map(Mini::page)
    }

    /// The person moved the mini: that is where it goes next time.
    pub(super) fn mini_moved(&self, shared: &Shared) {
        if let Some(at) = self.mini.as_ref().and_then(Mini::at) {
            crate::lock(&shared.kept).mini = Some(at);
        }
    }

    /// The person moved or sized the expanded window: that is its frame next time.
    pub(super) fn deck_moved(&self, deck: &Deck, event: &WindowEvent<'_>, shared: &Shared) {
        let moved = matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_));
        if !moved || self.presence.state() != State::Expanded {
            return;
        }
        let native = &deck.frame.native;
        let scale = native.scale_factor();
        if let Ok(at) = native.outer_position() {
            let (at, size) = (
                at.to_logical::<f64>(scale),
                native.outer_size().to_logical::<f64>(scale),
            );
            crate::lock(&shared.kept).deck = Some(Rect {
                x: at.x,
                y: at.y,
                width: size.width,
                height: size.height,
            });
        }
    }
}
