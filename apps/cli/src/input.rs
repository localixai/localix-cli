//! What a run is asked: the prompt from the command line and stdin, as `codex exec` reads them,
//! and the `--schema` its answer must match.

use std::io::{IsTerminal, Read};

use serde_json::Value;

use crate::error::{self, Failure};

/// Bytes of stdin read into a prompt: a large file piped in by mistake must not become a request
/// no model accepts. The agent can read a file whole with its tools instead.
const STDIN_LIMIT: usize = 256 * 1024;

/// The task to run, or the prompt line when there is none and a person is at the terminal.
#[derive(Debug, PartialEq, Eq)]
pub enum Prompt {
    One(String),
    Line,
}

/// The prompt from `arg` and stdin: `-` is stdin itself; a prompt with piped stdin gets it as a
/// `<stdin>` block; no prompt reads stdin, unless it is a terminal, which opens the prompt line.
pub fn prompt(arg: Option<&str>) -> Result<Prompt, Failure> {
    let stdin = Stdin::now();
    let piped = match (arg, stdin) {
        (Some("-"), _) | (_, Stdin::Stream | Stdin::File) => {
            if let Some(note) = waiting(arg, stdin) {
                error::note(note);
            }
            Some(read_stdin()?)
        }
        _ => None,
    };
    assemble(arg, piped.as_deref())
}

/// What stdin is, as far as waiting on it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stdin {
    Terminal,
    /// A pipe or a socket, which another process may hold open: reading it can wait unseen.
    Stream,
    /// A file or a device such as `/dev/null`, which always ends.
    File,
}

impl Stdin {
    fn now() -> Self {
        if std::io::stdin().is_terminal() {
            Self::Terminal
        } else if ends() {
            Self::File
        } else {
            Self::Stream
        }
    }
}

/// Whether stdin is something that always ends: not a pipe, a socket or unknown.
#[cfg(unix)]
fn ends() -> bool {
    use std::os::fd::AsFd;
    use std::os::unix::fs::FileTypeExt;
    let Ok(fd) = std::io::stdin().as_fd().try_clone_to_owned() else {
        return false;
    };
    match std::fs::File::from(fd).metadata() {
        Ok(meta) => {
            let kind = meta.file_type();
            !(kind.is_fifo() || kind.is_socket())
        }
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn ends() -> bool {
    false
}

/// What to tell the person before stdin is read to its end, where that could wait unseen: a
/// prompt given with stdin left open (a harness's pipe, `ssh` without `-n`, a `while read` loop),
/// or `-` at a terminal. Codex says the same.
pub fn waiting(arg: Option<&str>, stdin: Stdin) -> Option<&'static str> {
    match (arg, stdin) {
        (Some("-"), Stdin::Terminal) => {
            Some("lx: reading the prompt from stdin; end it with Ctrl-D\n")
        }
        (Some("-"), _) | (None, _) => None,
        (Some(_), Stdin::Stream) => {
            Some("lx: reading additional input from stdin (</dev/null to skip it)\n")
        }
        _ => None,
    }
}

/// The prompt from `arg` and what was read from stdin (`None`: stdin is a terminal, not read).
pub fn assemble(arg: Option<&str>, stdin: Option<&str>) -> Result<Prompt, Failure> {
    if arg.is_none() && stdin.is_none() {
        return Ok(Prompt::Line);
    }
    let piped = stdin.map(str::trim).filter(|text| !text.is_empty());
    let text = match (arg, piped) {
        (Some("-") | None, Some(text)) => text.to_string(),
        (Some("-") | None, None) => {
            return Err(Failure::Usage(
                "no prompt: lx \"<task>\", or pipe one in (stdin was empty)".into(),
            ));
        }
        (Some(prompt), Some(text)) => format!("{}\n\n<stdin>\n{text}\n</stdin>", prompt.trim()),
        (Some(prompt), None) => prompt.trim().to_string(),
    };
    if text.trim().is_empty() {
        return Err(Failure::Usage("the prompt is empty".into()));
    }
    Ok(Prompt::One(text))
}

fn read_stdin() -> Result<String, Failure> {
    let mut bytes = vec![];
    std::io::stdin()
        .lock()
        .take(STDIN_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| Failure::Environment(format!("could not read stdin: {error}")))?;
    let cut = bytes.len() > STDIN_LIMIT;
    bytes.truncate(STDIN_LIMIT);
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if cut {
        text.push_str(&format!(
            "\n[stdin cut at {} KiB: the rest was not read]",
            STDIN_LIMIT / 1024
        ));
    }
    Ok(text)
}

/// `--schema`: inline JSON, or `@path` to a file; it must be a JSON Schema object.
pub fn schema(arg: &str) -> Result<Value, Failure> {
    let usage = |message: String| Failure::Usage(message);
    let text = match arg.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|error| usage(format!("--schema {path}: {error}")))?,
        None => arg.to_string(),
    };
    let schema: Value = serde_json::from_str(&text)
        .map_err(|error| usage(format!("--schema is not JSON: {error}")))?;
    if !schema.is_object() {
        return Err(usage("--schema must be a JSON Schema object".into()));
    }
    Ok(schema)
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
