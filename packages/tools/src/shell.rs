//! The shell tool: [`BashTool`] (`bash`) runs a command in the workspace, after asking the gate,
//! and reports its exit code and output.

mod bash;
#[cfg(unix)]
mod inherit;
mod output;
mod process;
#[cfg(unix)]
mod sweep;

pub use bash::BashTool;
#[cfg(unix)]
pub use inherit::streams_only;
pub use process::{Ending, Env, SHELL, kill, shell};
