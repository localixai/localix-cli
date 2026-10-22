//! The shell tool: [`BashTool`] (`bash`) runs a command in the workspace, after asking the gate,
//! and reports its exit code and output.

mod bash;
mod output;
mod process;

pub use bash::BashTool;
pub use process::SHELL;
