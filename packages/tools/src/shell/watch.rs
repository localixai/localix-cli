//! A session's window, which the app opens through a [`Watch`]: the shell gives it a title and
//! paints frames of the session's screen into it (see `screen`), and knows nothing of what shows
//! them. A window opens once a session is kept, follows its screen, and closes when a call ends
//! the session; a program that ends by itself leaves its window a moment, titled with how it
//! ended.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::screen::Screen;
pub use super::stream::Painter;
use super::stream::Shared;

/// A window for a session: its title in, a painter out, or none when no window can open.
pub type Watch = Arc<dyn Fn(u32, &str) -> Option<Painter> + Send + Sync>;
/// How long a session that ended by itself keeps its window.
pub const LINGER: Duration = Duration::from_secs(3);
/// How long the end's status is looked for once the terminal closed.
const STATUS_WAIT: Duration = Duration::from_millis(200);

/// A session's window title.
pub fn title(id: u32, command: &str) -> String {
    format!("session {id} · {command}")
}

/// The terminal closed: the window says how the program ended (`status`), then closes after
/// [`LINGER`], or at once when a call closes it.
pub fn linger(shared: &Shared, status: Option<i32>, painter: Painter, screen: &Screen) {
    let end = match status {
        Some(code) => format!(" · exited {code}"),
        None => " · exited".to_string(),
    };
    if !painter(screen.frame(Some(&end))) {
        return;
    }
    let stream = shared.lock();
    let kept = shared
        .wake
        .wait_timeout_while(stream, LINGER, |stream| !stream.dismissed);
    drop(kept);
    drop(painter);
}

/// How `pid` ended: its exit code, or 128 and the signal that killed it. It is not reaped.
pub fn status(pid: u32) -> Option<i32> {
    let deadline = Instant::now() + STATUS_WAIT;
    loop {
        // SAFETY: waitid only writes into `info`, which is zeroed and lives through the call.
        let (done, info) = unsafe {
            let mut info: libc::siginfo_t = std::mem::zeroed();
            let flags = libc::WEXITED | libc::WNOHANG | libc::WNOWAIT;
            let done = libc::waitid(libc::P_PID, pid as libc::id_t, &mut info, flags);
            (done, info)
        };
        if done != 0 {
            return None;
        }
        if info.si_signo == libc::SIGCHLD {
            // SAFETY: a SIGCHLD's info carries the status.
            let code = unsafe { info.si_status() };
            return Some(match info.si_code {
                libc::CLD_EXITED => code,
                _ => 128 + code,
            });
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
