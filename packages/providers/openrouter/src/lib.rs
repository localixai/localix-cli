//! A model provider: OpenRouter's chat models behind the chat port ([`chat::ChatModel`]) and its
//! decision models behind the decision port ([`system_one::SystemOne`]), its HTTP client, a
//! catalog of chat models with the reasoning levels each takes, and its key: from the
//! environment, pasted, or issued by a sign-in. It knows nothing of the app that uses it beyond
//! the [`App`] it is given.

pub mod auth;
pub mod catalog;
pub mod chat;
pub mod client;
pub mod system_one;

pub use provider_kit::App;

/// How the provider is named to the person.
pub const NAME: &str = "OpenRouter";
