//! The shell tool: [`BashTool`] (`bash`) runs a command in the workspace, after asking the gate,
//! and reports its exit code and output. On unix a command may instead run in a terminal of its
//! own, a session kept in [`Sessions`] between calls, for a program that must ask.

mod bash;
#[cfg(unix)]
mod filter;
mod interactive;
#[cfg(unix)]
mod keys;
mod output;
mod process;
#[cfg(unix)]
mod pty;
#[cfg(unix)]
mod report;
#[cfg(unix)]
mod running;
#[cfg(unix)]
mod screen;
#[cfg(unix)]
mod session;
#[cfg(unix)]
mod stream;
#[cfg(unix)]
mod sweep;
#[cfg(unix)]
mod watch;

pub use bash::BashTool;
#[cfg(not(unix))]
pub use none::{Sessions, kill_all_now, running_anywhere};
pub use process::{Ending, SHELL};
#[cfg(unix)]
pub use running::{kill_all_now, running_anywhere};
#[cfg(unix)]
pub use screen::{Asset, Emulator, Rgb, Theme, page};
#[cfg(unix)]
pub use session::{MAX_SESSIONS, Sessions, Turn, Until, Wait};
#[cfg(unix)]
pub use watch::{Painter, Watch};

/// Where there are no terminal sessions: nothing to keep, close or kill.
#[cfg(not(unix))]
mod none {
    #[derive(Default)]
    pub struct Sessions {
        _none: (),
    }

    impl Sessions {
        pub fn close_all(&self) -> usize {
            0
        }

        pub fn running(&self) -> String {
            "none".into()
        }
    }

    pub fn kill_all_now() {}

    pub fn running_anywhere() -> usize {
        0
    }
}
