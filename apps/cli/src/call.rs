//! The tool clients' interface (ARCHITECTURE.md W9; L7, checked): `llx window`, one call of the
//! agent's `window` from another agent's shell, as the daemon's client. It knows only the
//! protocol and the client: it says where it is and whose tabs it calls (its owner), sends one
//! command, and writes what it came to on stdout for the agent to read, a picture to a file it
//! names there.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use client::{CallError, Client};
use protocol::{Command, Done, Place};
use serde_json::Value;

/// How long the daemon has to take the handshake, list the tool or end the owner's tabs.
const ASK: Duration = Duration::from_secs(10);
/// How long a call may take: a goal of the decision model is the longest.
const CALL: Duration = Duration::from_secs(30 * 60);

/// What the agent asks with this process.
pub(crate) enum Asked {
    /// How to call it: the tool's description and parameters.
    Describe,
    /// One call, with the tool's arguments.
    Call(Value),
    /// The owner is done: its tabs close.
    Release,
}

/// How what was asked ended.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Came {
    Done,
    /// The call failed, and the output says why.
    Failed,
}

/// Where this interface is and whose tabs it calls: its command line is `window`, then `owner`.
pub(crate) fn here(owner: &str) -> Place {
    let dir = std::env::current_dir().unwrap_or_default();
    client::place::here(&dir, None, vec!["window".into(), owner.to_string()])
}

/// `asked`, from `place`, of the daemon `client` is connected to, its answer written to `out`
/// and a picture into `shots`; why it could not be asked.
pub(crate) fn run(
    client: &Client,
    place: Place,
    build: String,
    asked: Asked,
    shots: &Path,
    out: &mut impl Write,
) -> Result<Came, String> {
    let hello = Command::Initialize {
        version: protocol::VERSION,
        place: Some(place),
        build,
    };
    client.call(hello, ASK).map_err(|error| error.to_string())?;
    let (command, wait) = match asked {
        Asked::Describe => (Command::Tools, ASK),
        Asked::Release => (Command::Release, ASK),
        Asked::Call(arguments) => {
            let call = format!("window-{}", std::process::id());
            let name = "window".to_string();
            let call = Command::Tool {
                call,
                name,
                arguments,
            };
            (call, CALL)
        }
    };
    let written = match client.call(command, wait) {
        Ok(done) => said(done, shots, out),
        Err(CallError::Failed(why)) => writeln!(out, "{why}").map(|()| Came::Failed),
        Err(error @ (CallError::Gone | CallError::Timeout)) => return Err(error.to_string()),
    };
    written.map_err(|error| format!("the answer could not be written ({error})"))
}

/// What the daemon said, as the agent reads it.
fn said(done: Done, shots: &Path, out: &mut impl Write) -> io::Result<Came> {
    match done {
        Done::Tool {
            report,
            state,
            image,
        } => {
            writeln!(out, "{report}")?;
            if let Some(state) = state {
                writeln!(out, "\n{state}")?;
            }
            if let Some(image) = image {
                let kind = image.media_type.strip_prefix("image/").unwrap_or("jpeg");
                match saved(&image.data, kind, shots) {
                    Ok(path) => {
                        writeln!(out, "\nThe picture: {} (read it to see it)", path.display())?
                    }
                    Err(why) => writeln!(out, "\nThe picture was not saved: {why}")?,
                }
            }
            Ok(Came::Done)
        }
        Done::Tools { tools } => {
            for tool in tools {
                writeln!(out, "{}", USAGE)?;
                writeln!(out, "{}\n", tool.description)?;
                let parameters = serde_json::to_string_pretty(&tool.parameters)?;
                writeln!(out, "Its arguments, as a JSON Schema:\n{parameters}")?;
            }
            Ok(Came::Done)
        }
        Done::Ok => Ok(Came::Done),
        other @ (Done::Initialized { .. }
        | Done::Interrupted { .. }
        | Done::Work { .. }
        | Done::Attached { .. }
        | Done::Status { .. }
        | Done::Failed { .. }) => {
            writeln!(out, "the daemon answered something else: {other:?}")?;
            Ok(Came::Failed)
        }
    }
}

/// A picture's bytes, in base64, written to a new file in `shots`.
fn saved(data: &str, kind: &str, shots: &Path) -> Result<PathBuf, String> {
    let bytes = (base64::engine::general_purpose::STANDARD.decode(data))
        .map_err(|error| format!("not base64 ({error})"))?;
    std::fs::create_dir_all(shots).map_err(|error| error.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let path = shots.join(format!("{stamp}-{}.{kind}", std::process::id()));
    std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
    Ok(path)
}

/// What comes before the tool's own description: how a shell calls it.
const USAGE: &str = "llx window: one call of the `window` tool below per process, from your \
shell. Pass its arguments as one JSON object: llx window '{\"action\": \"open\", \"kind\": \
\"browser\", \"url\": \"https://example.com\"}' (`-` reads them from stdin). The tabs stay for \
their owner (the project, or --as NAME) between calls until `llx window --end`. The report is \
printed; a picture is saved to a file whose path is printed. Exit 0: done; 1: the call failed, \
and the output says why; 3: llx could not be reached or the arguments are not a JSON object.\n";

#[cfg(test)]
#[path = "call_tests.rs"]
mod tests;
