//! `bash`: a shell command run in the workspace, for what the file tools do not do: builds, tests,
//! git, search. The model reads the exit code and both streams; a failing command is an answer,
//! not an error.

use std::time::Duration;

use contracts::gate::Gate;
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde::Deserialize;
use serde_json::{Value, json};

use super::SHELL;
use super::process::{self, Ending, Finished, Job};
use crate::ask;
use crate::workspace::Workspace;

const NAME: &str = "bash";
const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;

/// What the model reads of the tool; `{shell}` is the shell this system runs commands with.
const DESCRIPTION: &str = "Run a shell command (`{shell}`) in the workspace root, or in `workdir`, \
and return its exit code, stdout and stderr. Use it for builds, tests, git, package managers and \
searching (`rg`, `rg --files`, `grep`, `find`); to read or change a file, use `read` and `edit` \
instead. Each call is a fresh shell: `cd` and variables do not carry over, so pass `workdir` or \
chain with `&&`. Input is closed and there is no terminal: never start an interactive program (an \
editor, a pager, a prompt); pass flags like `--yes` or `--no-pager`. The command is killed, with \
everything it started, after `timeout_ms` (default 120000, at most 600000): start a server in the \
background with its output sent to a file. Each stream keeps about 10000 characters, its start \
and its end. A non-zero exit is reported as output, not as a failure.";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    command: String,
    timeout_ms: Option<u64>,
    workdir: Option<String>,
}

pub struct BashTool<'a> {
    pub workspace: &'a Workspace,
    pub gate: &'a dyn Gate,
    /// Environment variables commands must not see: the app's own secrets.
    pub hidden: &'a [&'a str],
    /// Whether the person stopped the task: a running command is killed when it says so.
    pub stop: &'a dyn Fn() -> bool,
}

impl Tool for BashTool<'_> {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME.into(),
            description: DESCRIPTION.replace("{shell}", SHELL),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "The command to run."},
                    "timeout_ms": {"type": "integer", "minimum": 1, "maximum": MAX_TIMEOUT_MS,
                        "description": "Kill it after this many milliseconds (default 120000)."},
                    "workdir": {"type": "string",
                        "description": "The directory to run it in, relative to the workspace root."},
                },
                "required": ["command"],
                "additionalProperties": false,
            }),
        }
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let args: Args = serde_json::from_value(arguments.clone())
            .map_err(|error| ToolError::Failed(format!("unusable arguments: {error}")))?;
        if args.command.trim().is_empty() {
            return Err(ToolError::Failed("the command is empty".into()));
        }
        let workdir = args.workdir.as_deref().filter(|dir| !dir.trim().is_empty());
        let dir = match workdir {
            Some(dir) => self.workspace.readable(dir)?,
            None => self.workspace.root().to_path_buf(),
        };
        if !dir.is_dir() {
            return Err(ToolError::Failed(format!(
                "{} is not a directory",
                self.workspace.shown(&dir)
            )));
        }
        // The person judges where it runs as much as what runs: `rm -rf *` in `~` is not the same
        // command as in the workspace.
        let place = workdir.map(|_| self.place(&dir));
        let effect = match &place {
            Some(place) => format!("{}  (in {place})", summary(&args.command)),
            None => summary(&args.command),
        };
        ask(self.gate, NAME, &effect)?;
        let timeout = args
            .timeout_ms
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .clamp(1, MAX_TIMEOUT_MS);
        let job = Job {
            command: &args.command,
            dir: &dir,
            timeout: Duration::from_millis(timeout),
            hidden: self.hidden,
            stop: self.stop,
        };
        let finished = process::run(&job)
            .map_err(|error| ToolError::Failed(format!("the shell could not start: {error}")))?;
        Ok(ToolOutput {
            report: report(&finished, place.as_deref()),
            state: None,
        })
    }
}

impl BashTool<'_> {
    /// `dir` as the gate and the report show it: relative inside the workspace, else absolute and
    /// said to be outside.
    fn place(&self, dir: &std::path::Path) -> String {
        let shown = self.workspace.shown(dir);
        match dir.starts_with(self.workspace.root()) {
            true => shown,
            false => format!("{shown}, outside the workspace"),
        }
    }
}

/// The whole command as the gate shows it, on one line: each line break becomes a return sign,
/// and nothing else is left out, so what is approved is what runs.
fn summary(command: &str) -> String {
    command
        .trim_end_matches(['\n', '\r'])
        .replace("\r\n", "\n")
        .replace('\n', " \u{23ce} ")
}

/// How it ended, then each stream that printed anything.
fn report(finished: &Finished, place: Option<&str>) -> String {
    let seconds = format!("{:.1}s", finished.elapsed.as_secs_f64());
    let mut head = match finished.ending {
        Ending::Exit(code) => format!("exit {code} in {seconds}"),
        Ending::Signal(signal) => format!("killed by signal {signal} in {seconds}"),
        Ending::TimedOut { killed } => format!(
            "timed out after {seconds}: {} (raise timeout_ms, at most {MAX_TIMEOUT_MS}, or run \
             it in the background)",
            killed_or_not(killed)
        ),
        Ending::Stopped { killed } => format!(
            "stopped by the person after {seconds}: {}",
            killed_or_not(killed)
        ),
        Ending::Unknown => format!("ended in {seconds}"),
    };
    if let Some(place) = place {
        head.push_str(&format!(" (in {place})"));
    }
    let mut out = vec![head];
    for (name, text) in [("stdout", &finished.stdout), ("stderr", &finished.stderr)] {
        if !text.is_empty() {
            out.push(format!("### {name}\n{text}"));
        }
    }
    if out.len() == 1 {
        out[0].push_str(", no output");
    }
    if finished.open {
        out.push(
            "(a background process still holds the output: what it prints later is not shown)"
                .into(),
        );
    }
    out.join("\n")
}

fn killed_or_not(killed: bool) -> &'static str {
    match killed {
        true => "the command and everything it started were killed",
        false => "the command could not be killed, and may still be running",
    }
}

#[cfg(test)]
#[path = "bash_tests.rs"]
mod tests;
