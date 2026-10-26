//! What the person does to the window itself: closes it, resizes it, or changes the system's
//! appearance under it.

use tao::event::WindowEvent;
use tao::window::WindowId;

use super::super::deck::Deck;
use super::super::frame;
use super::{Heard, Shared};

/// The person did something to the window itself.
pub(super) fn on_window(
    deck: &mut Option<Deck>,
    window_id: WindowId,
    event: &WindowEvent<'_>,
    shared: &Shared,
    on_close: fn(),
) {
    let Some(open) = deck.as_mut().filter(|d| d.frame.native.id() == window_id) else {
        return;
    };
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "tao's events: foreign, non-exhaustive"
    )]
    match event {
        // The person closed the window: the browser's calls fail and the task is told, and the
        // terminals' callers let their screens go.
        WindowEvent::CloseRequested => {
            open.let_go();
            let browser = open.web.is_some();
            *deck = None;
            shared.heard("window", Heard::Did(CLOSED_WINDOW));
            if browser {
                shared.closed();
                on_close();
            }
        }
        event if frame::relayout(event) => open.layout(),
        // The system's appearance changed: a glass that follows it is tinted again.
        WindowEvent::ThemeChanged(_) => open.frame.retint(),
        _ => {}
    }
}

/// What the person's closing says, to whoever listens.
const CLOSED_WINDOW: &str = "The person closed the window: its tabs are gone (terminal programs \
run on, out of sight).";
