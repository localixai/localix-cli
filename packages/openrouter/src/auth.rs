//! The OpenRouter key: `OPENROUTER_API_KEY`, else the one a sign-in saved for the app. Signing in is
//! OpenRouter's PKCE flow: it ends with a permanent API key, so nothing ever refreshes.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{App, auth_page};

const AUTHORIZE_URL: &str = "https://openrouter.ai/auth";
/// The environment variable a key comes from first.
pub const KEY_VARIABLE: &str = "OPENROUTER_API_KEY";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

/// `<config>/<app slug>/credentials`.
pub fn credentials_path(app: App) -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))?;
    Some(base.join(app.slug).join("credentials"))
}

/// Where the key comes from, for `doctor`.
pub enum KeySource {
    Env,
    Saved(PathBuf),
}

pub fn key(app: App) -> Option<(String, KeySource)> {
    if let Some(key) = std::env::var(KEY_VARIABLE)
        .ok()
        .filter(|key| !key.trim().is_empty())
    {
        return Some((key.trim().to_string(), KeySource::Env));
    }
    let path = credentials_path(app)?;
    let saved = std::fs::read_to_string(&path).ok()?;
    let key = saved.trim().to_string();
    (!key.is_empty()).then_some((key, KeySource::Saved(path)))
}

pub fn save(app: App, key: &str) -> std::io::Result<PathBuf> {
    let path = credentials_path(app).ok_or_else(|| std::io::Error::other("no home directory"))?;
    std::fs::create_dir_all(path.parent().expect("credentials live in a directory"))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&path)?;
    // The mode above applies only to a new file: one saved before with wider permissions (by
    // hand, or under another umask) is narrowed before the key goes in.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(key.as_bytes())?;
    Ok(path)
}

pub fn logout(app: App) -> std::io::Result<bool> {
    let Some(path) = credentials_path(app) else {
        return Ok(false);
    };
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// A PKCE verifier and its S256 challenge.
pub fn pkce() -> (String, String) {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("the OS has randomness");
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

/// The `code` in a pasted redirect URL, or the pasted code itself.
pub fn code_from_paste(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    let Some((_, query)) = input.split_once('?') else {
        return Some(input.to_string());
    };
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix("code="))
        .map(|code| code.split('#').next().unwrap_or(code).to_string())
}

/// Sign in with OpenRouter in the system browser and save the key it issues.
pub fn login(app: App) -> Result<PathBuf, String> {
    let (verifier, challenge) = pkce();
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    // OpenRouter sends no `state`; a random path keeps stray requests from completing sign-in.
    let mut nonce = [0u8; 12];
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    let path = format!("/oauth/callback/{}", URL_SAFE_NO_PAD.encode(nonce));
    let callback = format!("http://127.0.0.1:{port}{path}");
    let url = format!(
        "{AUTHORIZE_URL}?callback_url={}&code_challenge={challenge}&code_challenge_method=S256",
        encode(&callback)
    );
    eprintln!("Opening OpenRouter sign-in in your browser:\n  {url}");
    eprintln!(
        "If the browser is on another machine, paste the address it ends on (or the code) here."
    );
    open_browser(&url);

    // Either the browser comes back to the loopback address, or the person pastes where it ended.
    let (sender, receiver) = mpsc::channel::<Result<String, String>>();
    let from_browser = sender.clone();
    let browser_verifier = verifier.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut line = String::new();
            if BufReader::new(&stream).read_line(&mut line).is_err() {
                continue;
            }
            let target = line.split_whitespace().nth(1).unwrap_or("").to_string();
            if !target.starts_with(&path) {
                respond(
                    &stream,
                    "404 Not Found",
                    &auth_page::failure("Callback route not found.", None),
                );
                continue;
            }
            let result =
                match code_from_paste(&target) {
                    Some(code) => exchange(app, &code, &browser_verifier),
                    None => Err(query_value(&target, "error")
                        .unwrap_or_else(|| "no authorization code".into())),
                };
            match &result {
                Ok(_) => respond(&stream, "200 OK", &auth_page::success(app.name)),
                Err(error) => respond(
                    &stream,
                    "400 Bad Request",
                    &auth_page::failure("OpenRouter sign-in failed.", Some(error)),
                ),
            }
            let _ = from_browser.send(result);
            return;
        }
    });
    std::thread::spawn(move || {
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_ok()
            && let Some(code) = code_from_paste(&line)
        {
            let _ = sender.send(exchange(app, &code, &verifier));
        }
    });
    let key = receiver
        .recv_timeout(LOGIN_TIMEOUT)
        .map_err(|_| "sign-in timed out after 5 minutes".to_string())??;
    save(app, &key).map_err(|e| e.to_string())
}

fn respond(mut stream: &std::net::TcpStream, status: &str, page: &str) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{page}",
        page.len()
    );
}

fn query_value(target: &str, key: &str) -> Option<String> {
    let (_, query) = target.split_once('?')?;
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix(&format!("{key}=")).map(str::to_string))
}

fn exchange(app: App, code: &str, verifier: &str) -> Result<String, String> {
    let client = crate::client::OpenRouter::new(String::new(), app);
    let body = json!({"code": code, "code_verifier": verifier, "code_challenge_method": "S256"});
    let (value, _) = client
        .post("/auth/keys", &body)
        .map_err(|e| e.to_string())?;
    value
        .get("key")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "OpenRouter sent no API key".into())
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let command = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let command = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let command = std::process::Command::new("xdg-open").arg(url).spawn();
    let _ = command;
}

#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
