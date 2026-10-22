//! Exit codes: the contract scripts and test runners read.

/// The goal was reached and every expectation held.
pub const OK: i32 = 0;
/// The goal was not reached, or an expectation was false: the tested app failed.
pub const FAILED: i32 = 1;
/// Limits or the timeout ran out before an outcome: the run could not decide.
pub const EXHAUSTED: i32 = 2;
/// The environment is not ready: no key, no network, no WebView runtime, a bad argument.
pub const ENVIRONMENT: i32 = 3;
/// Interrupted by Ctrl-C, SIGTERM or closing the window.
pub const INTERRUPTED: i32 = 130;
