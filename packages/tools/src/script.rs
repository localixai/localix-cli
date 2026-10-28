//! `exec`: a short JavaScript script that chains the run's tools in one call (ARCHITECTURE.md
//! X1-X6). A chain where each call needs the last result, or a result far larger than what the
//! answer needs, costs one turn and sends back only what the script keeps.
//!
//! The script runs on QuickJS (`script/engine.rs`), on a thread of its own; every call it makes
//! comes back here and is carried out by the tool itself, the same instance the agent holds, so
//! the gate, the workspace and everything the app wraps around a tool stay as they are.

use std::time::{Duration, Instant};

use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde::Deserialize;
use serde_json::Value;

mod declare;
mod engine;
mod input;
mod runtime;

const NAME: &str = "exec";
const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
/// The pragma a script may start with: `// @exec: {"timeout_ms": 30000}`.
const PRAGMA: &str = "// @exec:";

/// What `exec` is for beside the other tools: a chain, not a single call.
const BESIDE: &str = "Run a short JavaScript script that calls the other tools, and return only \
what it keeps. Use it for a chain where each call needs the last result, or where a result is far \
larger than what you need: search, read the hits, keep the lines that matter; run the checks, \
return only the failures. Not for one call, nor for independent calls (make those together in one \
turn), nor to carry a file's whole content in a string.";

/// What `exec` is for when it is the only way to the tools.
const ALONE: &str = "Run a short JavaScript script that calls the tools below: they are reached \
only through one. A script may make one call or chain several, each using the last result, and \
returns only what it keeps: search, read the hits, keep the lines that matter; run the checks, \
return only the failures. Never carry a file's whole content in a string.";

const HOW: &str = "The source is plain JavaScript, not JSON; it runs as the body of an async \
function, so `await` and `return` work at its top. Each `tools.<name>(args)` returns a promise of \
the tool's text result; a failed call rejects with an Error you can catch. Calls started together \
run at the same time where the tool allows it (reading); the others run one at a time, in order. \
Use `await Promise.all([...])` for independent calls, `Promise.allSettled([...])` when some may \
fail and you want every result, and `Promise.race([...])` for the first to finish. Await every \
call before you return: the result of one still running is lost. `text(value)` adds a line to \
what comes back (there is no console), then whatever the script returns (a string as it is, \
anything else as JSON); a long result is cut in the middle. In a template literal, write its own \
backticks and `${` as \\` and \\${. Nothing carries over from one script to the next, and there is \
no file system, network, process or clock but the tools. A first line \
`// @exec: {\"timeout_ms\": 30000}` sets the time it may take (default 120000, at most 600000).";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pragma {
    timeout_ms: Option<u64>,
}

pub struct ExecTool<'a> {
    /// The tools a script calls: the run's own, `exec` left out.
    pub tools: Vec<&'a dyn Tool>,
    /// Whether the person stopped the task: a running script is ended when it says so.
    pub stop: &'a (dyn Fn() -> bool + Sync),
    pub offered: Offered,
}

/// How the model is offered `exec`: beside the tools it calls, or as the only way to them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offered {
    /// The model also calls the tools directly, and reads their descriptions there.
    Beside,
    /// The tools are reached only from a script: `exec` carries their descriptions.
    Alone,
}

impl Tool for ExecTool<'_> {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME.into(),
            description: self.description(),
            input: input::input(),
        }
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let source = input::source(arguments)?;
        let pragma = pragma(source)?;
        let timeout = pragma
            .timeout_ms
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS);
        let limits = engine::Limits {
            deadline: Instant::now() + Duration::from_millis(timeout),
            stop: self.stop,
        };
        let call = |name: &str, arguments: &Value| self.serve(name, arguments);
        let parallel =
            |name: &str| (self.tools.iter()).any(|t| t.spec().name == name && t.parallel());
        let serve = engine::Serve {
            call: &call,
            parallel: &parallel,
        };
        match engine::run(source, &self.names(), &limits, &serve) {
            Ok(said) => Ok(ToolOutput::from(said)),
            Err(engine::Ended::Fatal(error)) => Err(error),
            Err(engine::Ended::Threw(message)) => Err(ToolError::Failed(message)),
            Err(engine::Ended::Stopped) => Err(ToolError::Failed(
                "the script was stopped: the task was stopped".into(),
            )),
            Err(engine::Ended::TimedOut) => Err(ToolError::Failed(format!(
                "the script did not end within {timeout} ms and was ended; set a longer \
                 `timeout_ms`, or do less in one script"
            ))),
        }
    }
}

impl ExecTool<'_> {
    /// What it is for, then each tool a script calls, declared as TypeScript.
    fn description(&self) -> String {
        let tools = self.tools.iter().map(|tool| tool.spec());
        let tools: Vec<ToolSpec> = tools.filter(|spec| spec.name != NAME).collect();
        match self.offered {
            Offered::Beside => {
                let declared: Vec<String> = (tools.iter())
                    .map(|spec| {
                        let declared = declare::declaration(&spec.name, &spec.input.schema(), 1);
                        format!("  {declared}")
                    })
                    .collect();
                let declared = declared.join("\n");
                format!("{BESIDE}\n\n{HOW}\n\n```ts\ndeclare const tools: {{\n{declared}\n}};\n```")
            }
            Offered::Alone => {
                let each: Vec<String> = (tools.iter())
                    .map(|spec| {
                        let declared = declare::declaration(&spec.name, &spec.input.schema(), 0);
                        let about = &spec.description;
                        format!("### `tools.{}`\n{about}\n```ts\n{declared}\n```", spec.name)
                    })
                    .collect();
                format!("{ALONE}\n\n{HOW}\n\n{}", each.join("\n\n"))
            }
        }
    }

    fn names(&self) -> Vec<String> {
        (self.tools.iter())
            .map(|tool| tool.spec().name)
            .filter(|name| name != NAME)
            .collect()
    }

    /// One call a script made: carried out by the tool of that name, its text result returned.
    fn serve(&self, name: &str, arguments: &Value) -> Result<String, ToolError> {
        let tool = (self.tools.iter())
            .find(|tool| tool.spec().name == name && name != NAME)
            .ok_or_else(|| ToolError::Failed(format!("no tool `{name}`")))?;
        tool.call(arguments).map(|output| output.report)
    }
}

/// The settings of a first line `// @exec: {...}`, if the script starts with one.
fn pragma(source: &str) -> Result<Pragma, ToolError> {
    let first = source.lines().next().unwrap_or_default().trim_start();
    let Some(json) = first.strip_prefix(PRAGMA) else {
        return Ok(Pragma::default());
    };
    serde_json::from_str(json.trim())
        .map_err(|error| ToolError::Failed(format!("the `{PRAGMA}` line is not valid: {error}")))
}

#[cfg(test)]
#[path = "script_tests.rs"]
mod tests;
