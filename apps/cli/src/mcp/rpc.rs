//! MCP's messages (JSON-RPC 2.0, one per line over stdio), read and written: what the other
//! agent asks, and the answers made from what the server said. No state, no I/O.

use client::CallError;
use protocol::{Done, ToolInfo};
use serde_json::{Value, json};

/// The MCP versions this server speaks, the newest first: a client asking for another is
/// answered with the newest, and decides whether to go on.
const VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// What the other agent is told of this server when it starts.
const INSTRUCTIONS: &str = "llx's `window`: native browser pages and terminal sessions as tabs \
of one window the person watches, grouped by project. Open a tab, act in it, read the snapshot \
each call returns; hand a short `goal` to llx's fast helper for a screen's worth of clicks or \
keys. Tabs end when this server does.";

pub(super) const PARSE_ERROR: i64 = -32700;
pub(super) const INVALID_REQUEST: i64 = -32600;
pub(super) const NOT_FOUND: i64 = -32601;
pub(super) const INVALID_PARAMS: i64 = -32602;

/// A line from the other agent.
#[derive(Debug, PartialEq)]
pub(super) enum Message {
    /// Answered by a response with its id.
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// Answered by nothing.
    Notification { method: String, params: Value },
    /// A response to a request of ours: this server sends none, so it is nothing to it.
    Response,
    /// Not JSON, or not a message: answered with a parse error.
    Unreadable,
}

pub(super) fn read(line: &str) -> Message {
    let Ok(Value::Object(mut message)) = serde_json::from_str::<Value>(line) else {
        return Message::Unreadable;
    };
    let params = message.remove("params").unwrap_or(Value::Null);
    let method = match message.remove("method") {
        Some(Value::String(method)) => method,
        Some(_) => return Message::Unreadable,
        None => return Message::Response,
    };
    match message.remove("id") {
        Some(id) => Message::Request { id, method, params },
        None => Message::Notification { method, params },
    }
}

pub(super) fn result(id: &Value, result: &Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string()
}

pub(super) fn error(id: &Value, code: i64, message: &str) -> String {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}).to_string()
}

/// What `initialize` comes to: the version asked when this server speaks it, else its newest.
pub(super) fn initialized(params: &Value, version: &str) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let speaks = asked.filter(|asked| VERSIONS.contains(asked));
    json!({
        "protocolVersion": speaks.unwrap_or(VERSIONS[0]),
        "capabilities": {"tools": {"listChanged": false}},
        "serverInfo": {"name": "llx", "version": version},
        "instructions": INSTRUCTIONS,
    })
}

pub(super) fn tools(tools: Vec<ToolInfo>) -> Value {
    let listed: Vec<Value> = (tools.into_iter())
        .map(|tool| {
            json!({"name": tool.name, "description": tool.description,
                "inputSchema": tool.parameters})
        })
        .collect();
    json!({"tools": listed})
}

/// A tool's call as the other agent's model reads it: the report, then the state the call left,
/// and its picture. A failure is a result too, for the model to read and do better.
pub(super) fn called(called: Result<Done, CallError>) -> Value {
    let (report, state, image) = match called {
        Ok(Done::Tool {
            report,
            state,
            image,
        }) => (report, state, image),
        Ok(other) => return failed(&format!("the server answered {other:?}")),
        Err(why) => return failed(&why.to_string()),
    };
    let text = match state {
        Some(state) => format!("{report}\n\n{state}"),
        None => report,
    };
    let mut content = vec![json!({"type": "text", "text": text})];
    if let Some(image) = image {
        content.push(json!({"type": "image", "data": image.data, "mimeType": image.media_type}));
    }
    json!({"content": content, "isError": false})
}

fn failed(why: &str) -> Value {
    json!({"content": [{"type": "text", "text": why}], "isError": true})
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
