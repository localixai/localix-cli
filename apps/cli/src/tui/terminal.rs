//! The terminal itself: its modes, the keys read from it, and the person's editor, which has it
//! for a while.

use std::io::{self, Write};
use std::sync::mpsc::Sender;
use std::time::Duration;

use crossterm::event::{Event as Terminal, KeyEventKind};

use super::event::Input;

/// Bracketed paste on, and keys reported so Shift+Enter differs from Enter where the terminal can.
pub(super) const ENTER: &str = "\x1b[?2004h\x1b[>1u";
const LEAVE: &str = "\x1b[<1u\x1b[?2004l\x1b[?25h";

/// The terminal as it was: cooked, the cursor shown, nothing more reported.
pub(crate) fn restore() {
    let _ = crossterm::terminal::disable_raw_mode();
    let mut out = io::stdout();
    let _ = out.write_all(LEAVE.as_bytes()).and_then(|()| out.flush());
}

/// The editor has the terminal: the keys are its, not the interface's.
static PAUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[allow(
    clippy::disallowed_methods,
    reason = "a poll: while the editor holds the terminal no event says it gave it back"
)]
pub(super) fn read_keys(events: &Sender<Input>) {
    use std::sync::atomic::Ordering;
    loop {
        if PAUSED.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }
        match crossterm::event::poll(Duration::from_millis(50)) {
            Ok(true) if !PAUSED.load(Ordering::SeqCst) => {}
            Ok(_) => continue,
            Err(_) => return,
        }
        let event = match crossterm::event::read() {
            Ok(Terminal::Key(key)) if key.kind != KeyEventKind::Release => Input::Key(key),
            Ok(Terminal::Paste(text)) => Input::Paste(text),
            Ok(Terminal::Resize(width, height)) => Input::Resize(width, height),
            Ok(_) => continue,
            Err(_) => return,
        };
        if events.send(event).is_err() {
            return;
        }
    }
}

/// The composer's `text` written in `$VISUAL` or `$EDITOR` (else `vi`), the terminal handed over
/// meanwhile; the text as saved, or as it was when the editor could not run.
#[allow(
    clippy::disallowed_methods,
    reason = "the person's own editor, with no deadline by design: it ends when they leave it; \
              and the reader's pause has no event to wait on"
)]
pub(super) fn editor(text: &str) -> String {
    let path = std::env::temp_dir().join(format!("llx-prompt-{}.md", std::process::id()));
    if std::fs::write(&path, text).is_err() {
        return text.to_string();
    }
    PAUSED.store(true, std::sync::atomic::Ordering::SeqCst);
    // The reader may be between a poll and its read: give it the time to see the pause.
    std::thread::sleep(Duration::from_millis(60));
    restore();
    let command = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());
    let ran = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{command} \"$0\""))
        .arg(&path)
        .status();
    let _ = crossterm::terminal::enable_raw_mode();
    let mut out = io::stdout();
    let _ = out.write_all(ENTER.as_bytes()).and_then(|()| out.flush());
    PAUSED.store(false, std::sync::atomic::Ordering::SeqCst);
    let edited = ran
        .ok()
        .filter(|status| status.success())
        .and_then(|_| std::fs::read_to_string(&path).ok());
    let _ = std::fs::remove_file(&path);
    edited.unwrap_or_else(|| text.to_string())
}
