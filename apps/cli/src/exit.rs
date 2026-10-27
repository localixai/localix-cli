//! Exit codes: the contract scripts, CI jobs and other agents read. Changing one is a breaking
//! change (see CHANGELOG.md).

/// The goal was reached (or the agent answered without saying it was not).
pub(crate) const OK: i32 = 0;
/// The agent answered that the goal was not reached, or the data did not match `--schema`.
pub(crate) const FAILED: i32 = 1;
/// Turns or the time limit ran out before an answer.
pub(crate) const EXHAUSTED: i32 = 2;
/// The environment or the command line is not ready: no key, no network, a bad argument.
pub(crate) const ENVIRONMENT: i32 = 3;
/// Interrupted: Ctrl-C, or SIGTERM.
pub(crate) const INTERRUPTED: i32 = 130;
