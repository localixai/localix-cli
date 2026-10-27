//! The mini (ARCHITECTURE.md A9; only macOS builds one, where `presence` has it): a small floating panel in a screen corner over every
//! Space, with a live picture of the tab shown, its project and id, a spinner while the agent
//! acts, and a highlight when it shows a tab. Its page (mini.html) is data in and two words out,
//! `expand` (a click) and `drag`; it never takes the keyboard or navigates.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine;
use serde_json::json;
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event_loop::{EventLoopProxy, EventLoopWindowTarget};
use tao::window::{Window, WindowBuilder, WindowId};
use wry::{NewWindowResponse, WebView, WebViewBuilder};

use super::capture::Shot;
use super::frame;
use super::host::Cmd;
use super::placement::MINI;
use super::platform::{adopt_mini, hide_mini, keep_tab_rendering, show_mini};
use super::presence::Event;
use super::tab_view::build;

/// The mini's page.
const PAGE: &str = include_str!("mini.html");

pub(super) struct Mini {
    window: Window,
    webview: WebView,
}

impl Mini {
    /// The mini at `at` (its top left), out of sight until [`Mini::show`].
    pub(super) fn new(
        target: &EventLoopWindowTarget<Cmd>,
        proxy: &EventLoopProxy<Cmd>,
        at: (f64, f64),
    ) -> Result<Self, String> {
        let window = WindowBuilder::new()
            .with_title("llx")
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_focused(false)
            .with_visible(false)
            .with_inner_size(LogicalSize::new(MINI.0, MINI.1))
            .with_position(LogicalPosition::new(at.0, at.1))
            .build(target)
            .map_err(|error| error.to_string())?;
        // A panel before its page is built (see `Frame::build`).
        adopt_mini(&window);
        let first = AtomicBool::new(true);
        let said = proxy.clone();
        let builder = WebViewBuilder::new()
            .with_html(PAGE)
            .with_transparent(true)
            .with_incognito(true)
            .with_focused(false)
            .with_bounds(frame::fill(LogicalSize::new(MINI.0, MINI.1), 0.0, 0.0))
            .with_background_throttling(wry::BackgroundThrottlingPolicy::Disabled)
            // Its own page only.
            .with_navigation_handler(move |_| first.swap(false, Ordering::SeqCst))
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_download_started_handler(|_, _| false)
            .with_ipc_handler(move |request| {
                let command = match request.body().as_str() {
                    "expand" => Cmd::Presence(Event::Expand),
                    "drag" => Cmd::MiniDrag,
                    _ => return,
                };
                let _ = said.send_event(command);
            });
        let webview = build(builder, &window).map_err(|error| error.to_string())?;
        keep_tab_rendering(&webview);
        Ok(Self { window, webview })
    }

    pub(super) fn id(&self) -> WindowId {
        self.window.id()
    }

    pub(super) fn show(&self) {
        show_mini(&self.window);
    }

    pub(super) fn hide(&self) {
        hide_mini(&self.window);
    }

    /// The person presses on it to move it.
    pub(super) fn drag(&self) {
        let _ = self.window.drag_window();
    }

    pub(super) fn page(&self) -> &WebView {
        &self.webview
    }

    /// Where it is now, its top left in points.
    pub(super) fn at(&self) -> Option<(f64, f64)> {
        let at = self.window.outer_position().ok()?;
        let at = at.to_logical::<f64>(self.window.scale_factor());
        Some((at.x, at.y))
    }

    /// Show `shot`, the tab's picture.
    pub(super) fn picture(&self, shot: &Shot) {
        let data = base64::engine::general_purpose::STANDARD.encode(&shot.jpeg);
        self.call(&format!("P(\"data:image/jpeg;base64,{data}\")"));
    }

    /// Name the tab shown: its project and its id.
    pub(super) fn tab(&self, id: &str, group: &str) {
        self.call(&format!("T({})", json!({"id": id, "group": group})));
    }

    pub(super) fn busy(&self, on: bool) {
        self.call(&format!("B({on})"));
    }

    pub(super) fn blink(&self, long: Duration) {
        self.call(&format!("K({})", long.as_millis()));
    }

    fn call(&self, script: &str) {
        let _ = self.webview.evaluate_script(script);
    }
}
