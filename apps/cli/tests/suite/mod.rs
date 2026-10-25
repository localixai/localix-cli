//! The binary end to end, with no network. A run that reaches a model (`exec`, `sessions`,
//! `doctor`, the flows at a terminal in `tui`) thinks with a scripted model behind a local server;
//! the rest stop before one is called. Each test gets its own home, config and sessions
//! directories, so nothing of the person's is read. `repo` checks the repository itself.

mod decide;
mod doctor;
mod exec;
#[cfg(unix)]
mod mcp;
mod pipes;
mod repo;
mod sessions;
mod sign_in;
mod tui;
mod usage;
