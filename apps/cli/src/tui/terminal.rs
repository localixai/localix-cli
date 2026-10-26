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

/// Where frames are written: the terminal through a descriptor of its own, not std's stdout,
/// which is line-buffered: it writes a frame up to its last line break and keeps the rest (the
/// last row, the cursor's place, the update's end) for a second write at the flush. A frame is
/// one buffer (`Screen`), so it goes out in one write.
pub(super) fn out() -> Box<dyn Write> {
    #[cfg(unix)]
    if let Ok(own) = unbuffered(io::stdout()) {
        return Box::new(own);
    }
    Box::new(io::stdout())
}

/// A writer to `fd` that keeps nothing back.
#[cfg(unix)]
fn unbuffered(fd: impl std::os::fd::AsFd) -> io::Result<std::fs::File> {
    Ok(fd.as_fd().try_clone_to_owned()?.into())
}

/// The terminal as it was: cooked, the cursor shown, nothing more reported.
pub(crate) fn restore() {
    let _ = crossterm::terminal::disable_raw_mode();
    let mut out = io::stdout();
    let _ = out.write_all(LEAVE.as_bytes()).and_then(|()| out.flush());
}

/// The terminal's modes before the interface made it raw, for a signal handler to put back.
#[cfg(unix)]
static SAVED: std::sync::OnceLock<libc::termios> = std::sync::OnceLock::new();

/// How the process ends at once, given by the app; called from the signal handler, so it must
/// be async-signal-safe.
#[cfg(unix)]
static QUIT: std::sync::OnceLock<fn() -> !> = std::sync::OnceLock::new();

/// SIGTERM and SIGHUP end the process with `quit` once the terminal is given back: its modes as
/// they were, the cursor shown, nothing more reported (the client has no other handler; in
/// process, `interrupt::install` replaces SIGTERM's with stopping the task). Called before the
/// terminal is made raw, whose modes it keeps.
#[cfg(unix)]
pub(super) fn on_signals(quit: fn() -> !) {
    extern "C" fn given_back(_: libc::c_int) {
        if let Some(modes) = SAVED.get() {
            // SAFETY: tcsetattr is async-signal-safe and only reads `modes`, which is static.
            unsafe { libc::tcsetattr(0, libc::TCSANOW, modes) };
        }
        // SAFETY: write is async-signal-safe, and LEAVE is static.
        unsafe { libc::write(1, LEAVE.as_ptr().cast(), LEAVE.len()) };
        if let Some(quit) = QUIT.get() {
            quit();
        }
        // SAFETY: _exit is async-signal-safe. Not reached: `quit` is set before the handler is.
        unsafe { libc::_exit(1) };
    }
    let _ = QUIT.set(quit);
    // SAFETY: isatty only looks at the descriptor.
    if unsafe { libc::isatty(0) } == 1 {
        // SAFETY: a termios is integers and arrays of them: all zero is one, for tcgetattr to fill.
        let mut modes: libc::termios = unsafe { std::mem::zeroed() };
        // SAFETY: tcgetattr writes only `modes`, which outlives the call.
        if unsafe { libc::tcgetattr(0, &mut modes) } == 0 {
            let _ = SAVED.set(modes);
        }
    }
    // SAFETY: a sigaction is integers, a mask and a handler's address: all zero is one, with no
    // handler, until it is set below.
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = given_back as extern "C" fn(libc::c_int) as libc::sighandler_t;
    // SAFETY: sigemptyset writes only the mask, which lives here.
    unsafe { libc::sigemptyset(&mut action.sa_mask) };
    for signal in [libc::SIGTERM, libc::SIGHUP] {
        // SAFETY: the action is whole and outlives the call, which copies it; the handler does
        // only what a signal handler may (tcsetattr, write, kill, _exit).
        unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) };
    }
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

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
