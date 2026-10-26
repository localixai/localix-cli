//! The binary end to end, with no network. A run that reaches a model (`exec`, `sessions`,
//! `doctor`, the flows at a terminal in `tui`) thinks with a scripted model behind a local server;
//! the rest stop before one is called. Each test gets its own home, config and sessions
//! directories, so nothing of the person's is read. `repo` checks the repository itself.

mod decide;
mod doctor;
pub(crate) mod exec;
#[cfg(target_os = "linux")]
mod hardening;
#[cfg(all(unix, not(feature = "browser")))]
mod headless;
#[cfg(unix)]
mod mcp;
mod modes;
mod pipes;
mod repo;
mod rules;
mod sessions;
mod sign_in;
#[cfg(unix)]
mod startup;
mod tui;
mod usage;
