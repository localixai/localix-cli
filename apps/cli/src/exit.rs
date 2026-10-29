//! Exit codes: the contract scripts, CI jobs and other agents read. Changing one is a breaking
//! change (see CHANGELOG.md).

/// `result` said the goal was reached.
pub(crate) const OK: i32 = 0;
/// `result` said it was not, or its data did not match `--schema`, or the agent ended in text
/// without calling `result`.
pub(crate) const FAILED: i32 = 1;
/// Turns or the time limit ran out before an answer.
pub(crate) const EXHAUSTED: i32 = 2;
/// The environment or the command line is not ready: no key, no network, a bad argument.
pub(crate) const ENVIRONMENT: i32 = 3;
/// Interrupted: Ctrl-C, or SIGTERM.
pub(crate) const INTERRUPTED: i32 = 130;
