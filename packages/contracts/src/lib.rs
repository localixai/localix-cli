//! The contracts every part of the Localix CLI speaks, and nothing else: no logic, no technology.
//! - [`chat`]: the chat-model port: a conversation in, the next message out. A provider package
//!   implements [`chat::Chat`].
//! - [`tool`]: the agent's tools: a spec the model reads and a call that returns text. A tool
//!   package implements [`tool::Tool`].
//! - [`view`]: the view protocol: what a view shows (an observation of refs, roles and text) and
//!   what can be done to it (an action on a ref). A tool package implements [`view::View`].
//! - [`model`]: the decision-model port: typed questions answered with probabilities. A provider
//!   package implements [`model::Decider`].
//! - [`hands`]: a short task carried out on a view. The decision loop implements [`hands::Hands`].
//! - [`gate`]: a tool asks before an effect (a command, a file change); the app answers.
//! - [`person`]: a tool asks the person a question; the app answers, or says nobody can.
//! - [`history`]: the session-history port. A store package implements [`history::History`].

pub mod chat;
pub mod gate;
pub mod hands;
pub mod history;
pub mod model;
pub mod person;
pub mod tool;
pub mod view;
