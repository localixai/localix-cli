//! Test-only: tokens shaped as the issuer shapes them, and an HTTP fake that answers from a
//! script and records what was sent.

use std::sync::Mutex;

use provider_kit::http::{Http, Reply};
use serde_json::{Value, json};

/// A JWT with `claims` (the signature is not checked by the reader).
pub(crate) fn jwt(claims: Value) -> String {
    use base64::Engine;
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
    format!("h.{payload}.sig")
}

/// An access token for `account` that lapses at `exp`.
pub(crate) fn access(account: &str, exp: u64) -> String {
    jwt(json!({"exp": exp, "https://api.openai.com/auth": {"chatgpt_account_id": account}}))
}

/// One request as the fake saw it.
#[derive(Debug, Clone)]
pub(crate) struct Sent {
    pub address: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Sent {
    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// Answers each request with the next reply of its script.
#[derive(Default)]
pub(crate) struct Scripted {
    pub replies: Mutex<Vec<Reply>>,
    pub sent: Mutex<Vec<Sent>>,
}

impl Scripted {
    pub(crate) fn new(replies: Vec<(u16, String)>) -> Self {
        let replies = replies
            .into_iter()
            .rev()
            .map(|(status, body)| Reply { status, body })
            .collect();
        Self {
            replies: Mutex::new(replies),
            sent: Mutex::default(),
        }
    }

    pub(crate) fn sent(&self) -> Vec<Sent> {
        self.sent.lock().unwrap().clone()
    }
}

impl Http for Scripted {
    #[allow(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn post(&self, address: &str, headers: &[(&str, &str)], body: &str) -> Result<Reply, String> {
        self.sent.lock().unwrap().push(Sent {
            address: address.into(),
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: body.into(),
        });
        self.replies
            .lock()
            .unwrap()
            .pop()
            .ok_or_else(|| "no reply scripted".into())
    }
}
