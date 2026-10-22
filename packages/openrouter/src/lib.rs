//! A model provider: OpenRouter's chat models behind the chat port ([`chat::ChatModel`]) and its
//! decision models behind the decision port ([`system_one::SystemOne`]), its HTTP client, and
//! signing in for a key. It knows nothing of the app that uses it beyond the [`App`] it is given.

pub mod auth;
pub mod auth_page;
pub mod chat;
pub mod client;
pub mod system_one;

/// The app calling OpenRouter: how it is named in requests and on the sign-in page, and the folder
/// its key is saved in.
#[derive(Debug, Clone, Copy)]
pub struct App {
    /// "Localix CLI": OpenRouter's attribution and the sign-in page.
    pub name: &'static str,
    /// "lx": the user agent and the config folder.
    pub slug: &'static str,
    pub version: &'static str,
}
