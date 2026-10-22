//! The contracts every part of Decision Loop speaks, and nothing else: no loop, no technology.
//! - [`tool`]: the tool protocol: what a tool shows (an observation of refs, roles and text) and
//!   what can be done to it (an action on a ref). A tool package implements [`tool::Tool`].
//! - [`model`]: the decision-model port: typed questions answered with probabilities. A provider
//!   package implements [`model::Decider`].
//! - [`history`]: the session-history port. A store package implements [`history::History`].

pub mod history;
pub mod model;
pub mod tool;
