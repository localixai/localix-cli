//! The HTTP client to OpenRouter, and to any endpoint that speaks OpenAI's chat completions at
//! another address (DeepSeek, xAI, Groq, Mistral…: the catalog gives the address), over the
//! kit's [`Http`]. Its one agent keeps the connection alive across steps, so a decision costs a
//! request, not a handshake.

use std::time::{Duration, Instant};

use provider_kit::http::{Http, HttpError, Web};
use provider_kit::retry::{Attempt, PAUSE, TRANSIENT, retried};
use serde_json::Value;

pub const BASE_URL: &str = "https://openrouter.ai/api/v1";
/// A long reply with high reasoning is slow, and it is not streamed.
const TIMEOUT: Duration = Duration::from_secs(600);
/// A decision is short and cheap: one that hangs is given up on sooner, and asked once more.
const DECIDING: Duration = Duration::from_secs(90);

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ApiError {
    /// The key was refused: `/login` again, or fix the key's variable.
    Auth(String),
    /// The provider or the model answered with an error.
    Http(u16, String),
    /// No answer: offline, DNS, timeout.
    Network(String),
    /// An answer that does not have the expected shape.
    Invalid(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auth(message) => write!(
                f,
                "refused the key: {message}; type /login, or check the key's variable"
            ),
            Self::Http(status, message) => write!(f, "answered {status}: {message}"),
            Self::Network(message) => write!(f, "unreachable: {message}"),
            Self::Invalid(message) => write!(f, "unexpected answer: {message}"),
        }
    }
}

impl std::error::Error for ApiError {}

pub struct OpenRouter {
    http: Box<dyn Http>,
    key: String,
    base: String,
    app: crate::App,
    /// OpenRouter itself, wherever it is reached: it takes fields of its own.
    openrouter: bool,
    /// Who it calls, for errors: `OpenRouter`, `DeepSeek`.
    pub(crate) name: String,
    /// A request sent and not answered is sent once more (decisions); otherwise never.
    retry_timeout: bool,
}

/// What one call took, for the trace and the usage report.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Timing {
    pub millis: u64,
}

impl OpenRouter {
    pub fn new(key: String, app: crate::App) -> Self {
        Self::openrouter_at(std::env::var("OPENROUTER_BASE_URL").ok(), key, app)
    }

    /// OpenRouter at `base` (one standing in for it), else at its own address.
    fn openrouter_at(base: Option<String>, key: String, app: crate::App) -> Self {
        let base = base.unwrap_or_else(|| BASE_URL.into());
        let at = Self::at(&base, key, app, crate::NAME);
        Self {
            openrouter: true,
            ..at
        }
    }

    /// OpenRouter for the browser's decisions: a decision is short and cheap, so one that hangs
    /// is given up on sooner and asked once more.
    pub fn deciding(key: String, app: crate::App) -> Self {
        Self {
            http: Box::new(Web::new(app, DECIDING)),
            retry_timeout: true,
            ..Self::new(key, app)
        }
    }

    /// A client of the chat completions at `base`, called `name`.
    pub fn at(base: &str, key: String, app: crate::App, name: &str) -> Self {
        Self {
            http: Box::new(Web::new(app, TIMEOUT)),
            key,
            base: base.trim_end_matches('/').to_string(),
            app,
            openrouter: false,
            name: name.to_string(),
            retry_timeout: false,
        }
    }

    pub fn is_openrouter(&self) -> bool {
        self.openrouter
    }

    /// POST `body` to `path`, asked again on an answer a moment's wait may mend.
    pub(crate) fn post(&self, path: &str, body: &Value) -> Result<(Value, Timing), ApiError> {
        let started = Instant::now();
        let url = format!("{}{path}", self.base);
        let bearer = format!("Bearer {}", self.key);
        let headers = [
            ("Authorization", bearer.as_str()),
            ("X-Title", self.app.name),
            ("Content-Type", "application/json"),
        ];
        let body = body.to_string();
        let mut timed_out = false;
        retried(PAUSE, || {
            let (status, text) = match self.http.post(&url, &headers, &body) {
                Ok(reply) => (reply.status, reply.body),
                // A reply being written is billed: asking again would pay for it twice (a decision
                // is asked once more). One that never got through is asked again.
                Err(HttpError::Unanswered(why)) => {
                    if !self.retry_timeout || timed_out {
                        return Attempt::Done(Err(ApiError::Network(why)));
                    }
                    timed_out = true;
                    return Attempt::Again(ApiError::Network(why));
                }
                Err(HttpError::Unsent(why)) => return Attempt::Again(ApiError::Network(why)),
            };
            let parsed: Option<Value> = serde_json::from_str(&text).ok();
            let failed = |code: u16, message: String| {
                let error = ApiError::Http(code, message);
                if transient(code) {
                    Attempt::Again(error)
                } else {
                    Attempt::Done(Err(error))
                }
            };
            if status == 200 {
                // OpenRouter reports some upstream failures inside a 200 body.
                if let Some(error) = parsed.as_ref().and_then(|value| value.get("error")) {
                    let code = error
                        .get("code")
                        .and_then(Value::as_u64)
                        .and_then(|code| u16::try_from(code).ok())
                        .unwrap_or(502);
                    return failed(code, message_of(error));
                }
                let millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
                return Attempt::Done(
                    parsed
                        .map(|value| (value, Timing { millis }))
                        .ok_or_else(|| ApiError::Invalid("not JSON".into())),
                );
            }
            let message = parsed
                .as_ref()
                .and_then(|value| value.get("error"))
                .map(message_of)
                .unwrap_or_else(|| text.chars().take(300).collect());
            match status {
                401 | 403 => Attempt::Done(Err(ApiError::Auth(message))),
                _ => failed(status, message),
            }
        })
    }
}

/// Whether a wait may mend `status`: a 429 here is a rate limit of a moment, as are the
/// statuses every host means so.
fn transient(status: u16) -> bool {
    status == 429 || TRANSIENT.contains(&status)
}

fn message_of(error: &Value) -> String {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("no message");
    message.chars().take(400).collect()
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
