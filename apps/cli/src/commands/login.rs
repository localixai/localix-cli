//! `lx login`, `login --with-token`, `login status` and `logout`: the OpenRouter key, from
//! `OPENROUTER_API_KEY` (which wins) or the file a sign-in saved.

use std::io::{BufRead, IsTerminal, Read};

use openrouter::auth::{self, KeySource};

use crate::cli::{LoginAction, LoginArgs};
use crate::compose::APP;
use crate::error::{self, Failure};
use crate::exit;

pub fn login(args: LoginArgs) -> Result<i32, Failure> {
    match (args.action, args.with_token) {
        (Some(LoginAction::Status), _) => status(),
        (None, true) => with_token(),
        (None, false) => {
            let path = auth::login(APP).map_err(Failure::Environment)?;
            error::print(&format!(
                "Signed in. The key is saved in {}\n",
                path.display()
            ))?;
            Ok(exit::OK)
        }
    }
}

/// Save the key read from stdin: a pipe whole, a terminal one line, not shown as it is typed.
fn with_token() -> Result<i32, Failure> {
    let stdin = std::io::stdin();
    let mut text = String::new();
    let read = if stdin.is_terminal() {
        eprint_flush("Paste the OpenRouter key and press Enter (it is not shown): ");
        unseen(|| stdin.lock().read_line(&mut text).map(|_| ()))
    } else {
        stdin.lock().read_to_string(&mut text).map(|_| ())
    };
    read.map_err(|error| Failure::Environment(format!("could not read stdin: {error}")))?;
    let key = text.trim();
    if key.is_empty() || key.contains(|c: char| c.is_whitespace() || c.is_control()) {
        return Err(Failure::Usage(
            "no key on stdin: printenv OPENROUTER_API_KEY | lx login --with-token".into(),
        ));
    }
    let path = auth::save(APP, key)
        .map_err(|error| Failure::Environment(format!("could not save the key: {error}")))?;
    error::print(&format!(
        "Saved the key {} in {}\n",
        mask(key),
        path.display()
    ))?;
    if from_env() {
        eprint_flush("Note: OPENROUTER_API_KEY is set, and it wins over the saved key.\n");
    }
    Ok(exit::OK)
}

/// Where the key comes from; exit 0 when there is one, 3 when not.
fn status() -> Result<i32, Failure> {
    match auth::key(APP) {
        Some((key, KeySource::Env)) => {
            error::print(&format!(
                "Signed in with OPENROUTER_API_KEY: {}\n",
                mask(&key)
            ))?;
            Ok(exit::OK)
        }
        Some((key, KeySource::Saved(path))) => {
            error::print(&format!(
                "Signed in with the key saved in {}: {}\n",
                path.display(),
                mask(&key)
            ))?;
            Ok(exit::OK)
        }
        None => Err(Failure::SignedOut),
    }
}

pub fn logout() -> Result<i32, Failure> {
    let path = auth::credentials_path(APP);
    let removed = auth::logout(APP)
        .map_err(|error| Failure::Environment(format!("could not remove the key: {error}")))?;
    match (removed, path) {
        (true, Some(path)) => error::print(&format!("Signed out: removed {}\n", path.display()))?,
        _ => error::print("No saved key.\n")?,
    }
    if from_env() {
        eprint_flush("Note: OPENROUTER_API_KEY is still set; unset it to sign out fully.\n");
    }
    Ok(exit::OK)
}

/// Run `read` with the terminal's echo off, so a pasted key never stays on the screen or in its
/// scrollback. Ctrl-C is read as a character meanwhile (and refused as a key), so the process is
/// never killed with the echo still off; the terminal is put back as it was either way.
#[cfg(unix)]
fn unseen<T>(read: impl FnOnce() -> std::io::Result<T>) -> std::io::Result<T> {
    use std::mem::MaybeUninit;
    let fd = libc::STDIN_FILENO;
    let mut saved = MaybeUninit::<libc::termios>::uninit();
    // SAFETY: tcgetattr fills `saved` for a terminal; it is read only when that succeeded.
    if unsafe { libc::tcgetattr(fd, saved.as_mut_ptr()) } != 0 {
        return read();
    }
    let saved = unsafe { saved.assume_init() };
    let mut quiet = saved;
    quiet.c_lflag &= !(libc::ECHO | libc::ISIG);
    quiet.c_lflag |= libc::ECHONL;
    // SAFETY: both settings are whole termios values for this descriptor.
    unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &quiet) };
    let result = read();
    unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &saved) };
    result
}

/// Windows: the console keeps its echo (no std call turns it off).
#[cfg(not(unix))]
fn unseen<T>(read: impl FnOnce() -> std::io::Result<T>) -> std::io::Result<T> {
    read()
}

fn from_env() -> bool {
    std::env::var(auth::KEY_VARIABLE).is_ok_and(|key| !key.trim().is_empty())
}

/// A key as it may be shown: `sk-or-...abcd`, never more of it.
pub fn mask(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 16 {
        return "...".into();
    }
    let start: String = chars[..6].iter().collect();
    let end: String = chars[chars.len() - 4..].iter().collect();
    format!("{start}...{end}")
}

fn eprint_flush(text: &str) {
    use std::io::Write;
    let mut err = std::io::stderr();
    let _ = err.write_all(text.as_bytes()).and_then(|()| err.flush());
}

#[cfg(test)]
#[path = "login_tests.rs"]
mod tests;
