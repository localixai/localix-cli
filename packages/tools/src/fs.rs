//! The file tools: [`ReadTool`] (`read`) shows a file's lines numbered or a directory's entries;
//! [`PatchTool`] (`apply_patch`) carries out a patch: files changed, created, deleted or moved.
//! Both resolve paths through a [`Workspace`](crate::workspace::Workspace); only `apply_patch` asks
//! the gate. [`named_paths`] says which files a call of either names.

mod atomic;
mod diff;
mod patch;
pub mod picture;
mod read;
mod text;

pub use patch::PatchTool;
pub use patch::paths as named_paths;
pub use read::ReadTool;
