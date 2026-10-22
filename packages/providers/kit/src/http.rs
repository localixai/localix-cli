//! HTTP as the providers use it: a POST of a body with headers, answered with a status and the
//! whole body. Behind [`Http`], so a provider's requests are tested with a fake and no network.

use std::time::Duration;

use crate::App;

/// What a server answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub body: String,
}

pub trait Http: Send + Sync {
    /// POST `body` to `address` with `headers`; `Err` when no answer came (offline, DNS, timeout).
    fn post(&self, address: &str, headers: &[(&str, &str)], body: &str) -> Result<Reply, String>;
}

/// The longest body read: a long streamed reply is a few megabytes at most.
const BODY_LIMIT: u64 = 64 << 20;

/// An agent on the OS's own TLS and trust store (Security.framework, SChannel, OpenSSL): no
/// bundled TLS stack or root certificates in the binary, and corporate roots work. Statuses are
/// answers, not errors. One agent keeps its connections alive across requests.
pub fn agent(app: App, timeout: Duration) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(timeout))
        .user_agent(format!("{}/{}", app.slug, app.version))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .provider(ureq::tls::TlsProvider::NativeTls)
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build();
    ureq::Agent::new_with_config(config)
}

/// [`Http`] over the network.
#[derive(Clone)]
pub struct Web {
    agent: ureq::Agent,
}

impl Web {
    pub fn new(app: App, timeout: Duration) -> Self {
        Self {
            agent: agent(app, timeout),
        }
    }
}

impl Http for Web {
    fn post(&self, address: &str, headers: &[(&str, &str)], body: &str) -> Result<Reply, String> {
        let mut request = self.agent.post(address);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let mut response = request.send(body).map_err(|error| error.to_string())?;
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(BODY_LIMIT)
            .read_to_string()
            .map_err(|error| error.to_string())?;
        Ok(Reply { status, body })
    }
}

/// `pairs` as an `application/x-www-form-urlencoded` body.
pub fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

/// `text` percent-encoded for a query or a form: only unreserved characters stay as they are.
pub fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `text` with its percent escapes (and `+` as a space) decoded.
pub fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let escaped = bytes.get(at + 1..at + 3).and_then(|hex| {
            let hex = std::str::from_utf8(hex).ok()?;
            u8::from_str_radix(hex, 16).ok()
        });
        match (bytes[at], escaped) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
                continue;
            }
            (b'+', _) => out.push(b' '),
            (byte, _) => out.push(byte),
        }
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The value of `key` in the query of `target` (a path or a whole address), decoded.
pub fn query_value(target: &str, key: &str) -> Option<String> {
    let (_, query) = target.split_once('?')?;
    let query = query.split('#').next().unwrap_or(query);
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        (name == key).then(|| decode(value))
    })
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
