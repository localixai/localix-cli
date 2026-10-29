//! What the suites share: a home of their own and the binary run in it (`home`), a scripted model
//! behind a local server (`model`), and, for the flows at a terminal, a pseudo-terminal read as a
//! screen (`term`).

pub(crate) mod home;
pub(crate) mod model;
pub(crate) mod term;
