//! The one HTTP client: OpenRouter for both the chat model and the decision model. One agent keeps
//! the connection alive across steps, so a decision costs a request, not a handshake.

use std::time::{Duration, Instant};

use serde_json::Value;

pub const BASE_URL: &str = "https://openrouter.ai/api/v1";
const RETRY_STATUSES: [u16; 5] = [408, 429, 502, 503, 529];
const ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, PartialEq)]
pub enum ApiError {
    /// The key was refused: sign in again or fix `OPENROUTER_API_KEY`.
    Auth(String),
    /// OpenRouter or the model answered with an error.
    Http(u16, String),
    /// No answer: offline, DNS, timeout.
    Network(String),
    /// An answer that does not have the expected shape.
    Invalid(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auth(message) => write!(f, "OpenRouter refused the key: {message}"),
            Self::Http(status, message) => write!(f, "OpenRouter answered {status}: {message}"),
            Self::Network(message) => write!(f, "OpenRouter is unreachable: {message}"),
            Self::Invalid(message) => write!(f, "unexpected answer: {message}"),
        }
    }
}

impl std::error::Error for ApiError {}

pub struct OpenRouter {
    agent: ureq::Agent,
    key: String,
    base: String,
}

/// What one call took, for the trace and the usage report.
#[derive(Debug, Clone, Copy, Default)]
pub struct Timing {
    pub millis: u64,
}

impl OpenRouter {
    pub fn new(key: String) -> Self {
        let base = std::env::var("LXSURF_OPENROUTER_URL").unwrap_or_else(|_| BASE_URL.into());
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_global(Some(Duration::from_secs(90)))
            .user_agent(concat!("localix-surf/", env!("CARGO_PKG_VERSION")))
            // The OS's own TLS and trust store (Security.framework, SChannel, OpenSSL): no
            // bundled TLS stack or root certificates in the binary, and corporate roots work.
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .provider(ureq::tls::TlsProvider::NativeTls)
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
            key,
            base,
        }
    }

    /// POST `body` to `path`, retrying overload answers with backoff.
    pub fn post(&self, path: &str, body: &Value) -> Result<(Value, Timing), ApiError> {
        let started = Instant::now();
        let url = format!("{}{path}", self.base);
        let mut last = ApiError::Network("no attempt".into());
        for attempt in 0..ATTEMPTS {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(400 << attempt));
            }
            let sent = self
                .agent
                .post(&url)
                .header("Authorization", &format!("Bearer {}", self.key))
                .header("HTTP-Referer", "https://localix.ai")
                .header("X-Title", "Localix Surf")
                .send_json(body);
            let mut response = match sent {
                Ok(response) => response,
                Err(error) => {
                    last = ApiError::Network(error.to_string());
                    continue;
                }
            };
            let status = response.status().as_u16();
            let text = response.body_mut().read_to_string().unwrap_or_default();
            let parsed: Option<Value> = serde_json::from_str(&text).ok();
            if status == 200 {
                // OpenRouter reports some upstream failures inside a 200 body.
                if let Some(error) = parsed.as_ref().and_then(|value| value.get("error")) {
                    let code = error.get("code").and_then(Value::as_u64).unwrap_or(502) as u16;
                    last = ApiError::Http(code, message_of(error));
                    if RETRY_STATUSES.contains(&code) {
                        continue;
                    }
                    return Err(last);
                }
                let value = parsed.ok_or_else(|| ApiError::Invalid("not JSON".into()))?;
                return Ok((
                    value,
                    Timing {
                        millis: started.elapsed().as_millis() as u64,
                    },
                ));
            }
            let message = parsed
                .as_ref()
                .and_then(|value| value.get("error"))
                .map(message_of)
                .unwrap_or_else(|| text.chars().take(300).collect());
            last = match status {
                401 | 403 => return Err(ApiError::Auth(message)),
                _ => ApiError::Http(status, message),
            };
            if !RETRY_STATUSES.contains(&status) {
                return Err(last);
            }
        }
        Err(last)
    }

    pub fn get(&self, path: &str) -> Result<Value, ApiError> {
        let url = format!("{}{path}", self.base);
        let mut response = self
            .agent
            .get(&url)
            .header("Authorization", &format!("Bearer {}", self.key))
            .call()
            .map_err(|error| ApiError::Network(error.to_string()))?;
        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string().unwrap_or_default();
        match status {
            200 => {
                serde_json::from_str(&text).map_err(|error| ApiError::Invalid(error.to_string()))
            }
            401 | 403 => Err(ApiError::Auth(text.chars().take(300).collect())),
            _ => Err(ApiError::Http(status, text.chars().take(300).collect())),
        }
    }
}

fn message_of(error: &Value) -> String {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("no message");
    message.chars().take(400).collect()
}
