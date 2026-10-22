//! What the model providers share, so a new provider is mostly its wire format: an HTTP client on
//! the OS's own TLS ([`http`]), the pieces of an OAuth sign-in ([`oauth`], [`loopback`], [`page`]),
//! private credential files ([`store`]) and server-sent events ([`sse`]). It implements no port and
//! knows no provider.

pub mod http;
pub mod loopback;
pub mod oauth;
pub mod page;
pub mod sse;
pub mod store;

/// The app calling a provider: how it is named in requests and on the sign-in page, and the
/// folder its credentials are saved in.
#[derive(Debug, Clone, Copy)]
pub struct App {
    /// "llx": attribution and the sign-in page.
    pub name: &'static str,
    /// "llx": the user agent and the config folder.
    pub slug: &'static str,
    pub version: &'static str,
}
