//! The server's side of the seam (ARCHITECTURE.md K2, W1-W8): what it tells the interface and
//! how it says it. Everything here goes, with the sessions it serves, to `apps/server` (D1-D3).

pub(crate) mod label;
pub(crate) mod wire;
