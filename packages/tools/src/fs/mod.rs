//! The file tools: [`ReadTool`] (`read`) shows a file's lines numbered or a directory's entries;
//! [`EditTool`] (`edit`) changes a file by exact replacement, or creates one. Both resolve paths
//! through a [`Workspace`](crate::workspace::Workspace); only `edit` asks the gate.

mod atomic;
mod diff;
mod edit;
mod read;
mod text;

pub use edit::EditTool;
pub use read::ReadTool;
