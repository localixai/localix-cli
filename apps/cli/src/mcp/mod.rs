//! The tool clients' interface (ARCHITECTURE.md W9): `llx mcp` serves MCP over stdio to another
//! agent, as the daemon's client. It is an interface as the
//! terminal's is (L7): it knows only the protocol and the client. The daemon gives it a toolbox
//! at its place, the agent's own `window`; this side only turns MCP's messages into the
//! protocol's commands and their answers back. Each `tools/call` waits on a thread of its own,
//! so calls run at once; a cancelled one is halted and never answered.

mod link;
mod rpc;

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::time::Duration;

use client::{Begun, CallError, Client};
use protocol::{Command, Done, Place};
use serde_json::Value;

use self::link::Link;
use self::rpc::Message;

/// How long the daemon has to take the handshake and list the tools.
const ASK: Duration = Duration::from_secs(10);
/// How long a call may take: a goal of the decision model is the longest.
const CALL: Duration = Duration::from_secs(30 * 60);
/// Answers waiting for stdout, at most: a call that ends waits for the agent to read.
const WRITES: usize = 64;

/// Where this interface is: its tools work in its directory, with its environment, as a tool
/// client's (its command line is `mcp`).
pub(crate) fn here() -> Place {
    let env = std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)));
    let dir = std::env::current_dir().unwrap_or_default();
    Place {
        cwd: dir.display().to_string(),
        env: env.collect(),
        theme: None,
        args: vec!["mcp".into()],
        dir: dir.display().to_string(),
    }
}

/// Serve the agent at `input` and `output` through the daemon `connect` reaches (again when it
/// is lost), from `place`, until its input ends; then what its calls still do is halted. Why it
/// could not start.
pub(crate) fn run<F: Fn() -> io::Result<Client> + Sync>(
    connect: F,
    place: Place,
    build: String,
    input: impl BufRead,
    output: impl Write + Send,
) -> io::Result<()> {
    let link = Link::open(connect, place, build)?;
    let (lines, written) = sync_channel::<String>(WRITES);
    let open = Calls::default();
    std::thread::scope(|scope| {
        // Moved in: the writer ends once the last of the senders, the calls' too, is gone.
        let lines = lines;
        let writer = std::thread::Builder::new().name("llx-mcp-out".into());
        writer.spawn_scoped(scope, move || write(output, &written))?;
        for line in input.lines() {
            let Ok(line) = line else {
                break;
            };
            if line.trim().is_empty() {
                continue;
            }
            let lines = lines.clone();
            match rpc::read(&line) {
                Message::Request { id, method, params } if method == "tools/call" => {
                    // Known and sent before its thread starts, in the order the agent asked:
                    // a cancel that comes at once finds it, and reaches the server after it.
                    let Some(command) = open.start(&id, &params) else {
                        let unnamed = rpc::error(&id, rpc::INVALID_PARAMS, "a call names its tool");
                        let _ = lines.send(unnamed);
                        continue;
                    };
                    let begun = link.begin(command);
                    let (open, link) = (&open, &link);
                    let calling = move || answered(open, link, &id, begun, &lines);
                    let thread = std::thread::Builder::new().name("llx-mcp-call".into());
                    thread.spawn_scoped(scope, calling)?;
                }
                Message::Request { id, method, params } => {
                    let _ = lines.send(answer(&link, &id, &method, &params));
                }
                Message::Notification { method, params } => {
                    if method == "notifications/cancelled" {
                        open.cancel(&link.client().1, params.get("requestId"));
                    }
                }
                Message::Response => {}
                Message::Unreadable => {
                    let unread = rpc::error(&Value::Null, rpc::PARSE_ERROR, "not a message");
                    let _ = lines.send(unread);
                }
            }
        }
        // The agent is gone: nobody reads what its calls would say.
        open.halt_all(&link.client().1);
        Ok(())
    })
}

/// The calls running, by the agent's id for each, with the server's.
#[derive(Default)]
struct Calls {
    by_request: Mutex<HashMap<String, String>>,
    next: AtomicU64,
}

impl Calls {
    /// The call the request `id` asks for, known by its id at the server; none when it names
    /// no tool.
    fn start(&self, id: &Value, params: &Value) -> Option<Command> {
        let name = params.get("name").and_then(Value::as_str)?;
        let arguments =
            (params.get("arguments").cloned()).unwrap_or_else(|| Value::Object(Default::default()));
        let number = self.next.fetch_add(1, Ordering::Relaxed);
        let call = format!("mcp-{}-{number}", std::process::id());
        lock(&self.by_request).insert(id.to_string(), call.clone());
        Some(Command::Tool {
            call,
            name: name.to_string(),
            arguments,
        })
    }

    /// The request `id` is answered now, unless it was cancelled meanwhile.
    fn end(&self, id: &Value) -> bool {
        lock(&self.by_request).remove(&id.to_string()).is_some()
    }

    fn cancel(&self, client: &Client, id: Option<&Value>) {
        let call = id.and_then(|id| lock(&self.by_request).remove(&id.to_string()));
        if let Some(call) = call {
            let _ = client.send(Command::Halt { id: call });
        }
    }

    fn halt_all(&self, client: &Client) {
        let calls = std::mem::take(&mut *lock(&self.by_request));
        for call in calls.into_values() {
            let _ = client.send(Command::Halt { id: call });
        }
    }
}

/// A `tools/call`, waited for and answered, unless it was cancelled.
fn answered<F: Fn() -> io::Result<Client>>(
    open: &Calls,
    link: &Link<F>,
    id: &Value,
    begun: Result<Begun, CallError>,
    lines: &SyncSender<String>,
) {
    let seen = link.client().0;
    let done = begun.and_then(|begun| begun.wait(CALL));
    if done == Err(CallError::Gone) {
        link.lost(seen);
    }
    if open.end(id) {
        let _ = lines.send(rpc::result(id, rpc::called(done)));
    }
}

/// What a request other than a call comes to.
fn answer<F: Fn() -> io::Result<Client>>(
    link: &Link<F>,
    id: &Value,
    method: &str,
    params: &Value,
) -> String {
    match method {
        "initialize" => {
            let version = env!("CARGO_PKG_VERSION");
            rpc::result(id, rpc::initialized(params, version))
        }
        "ping" => rpc::result(id, Value::Object(Default::default())),
        "tools/list" => match link.call(Command::Tools, ASK) {
            Ok(Done::Tools { tools }) => rpc::result(id, rpc::tools(tools)),
            Ok(_) => rpc::error(id, rpc::INVALID_REQUEST, "the server lists no tools"),
            Err(why) => rpc::error(id, rpc::INVALID_REQUEST, &why.to_string()),
        },
        _ => rpc::error(id, rpc::NOT_FOUND, &format!("no method {method}")),
    }
}

/// Each answer as a line, flushed: the agent reads one message per line.
fn write(mut output: impl Write, written: &Receiver<String>) {
    for line in written {
        let wrote = writeln!(output, "{line}").and_then(|()| output.flush());
        if wrote.is_err() {
            return;
        }
    }
}

/// The calls' table, whatever a thread that held it did: it is only a map of ids.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
