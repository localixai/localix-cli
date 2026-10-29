//! The shell tool: [`BashTool`] (`bash`) runs a command in the workspace, after asking the gate,
//! and reports its exit code and output; a command the gate allows only to read runs in the
//! system's sandbox ([`Scope`]).

mod bash;
#[cfg(unix)]
mod inherit;
mod output;
mod process;
mod sandbox;
#[cfg(unix)]
mod sweep;

pub use bash::BashTool;
#[cfg(unix)]
pub use inherit::streams_only;
pub use process::{Env, SHELL, kill, scoped, shell};
pub use sandbox::{Held, Scope};
