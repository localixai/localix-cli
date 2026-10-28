//! A scripted model behind a real HTTP server:
//! the binary under test calls it as it calls OpenRouter (`OPENROUTER_BASE_URL`), it answers each
//! chat completion from a script, and it keeps every request for the test to read. No network.
//! A session's title is asked for beside its tasks: those requests are answered apart (with the
//! title set, else the task itself), never from the script, and kept apart.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

/// One answer of the model: text, tool calls, or an HTTP error with its status.
pub(crate) enum Reply {
    Text(String),
    Calls(Vec<(String, Value)>),
    Refused(u16),
    /// The reply given once the file exists: a lane's request waits while another works.
    After(std::path::PathBuf, Box<Reply>),
    /// A decision model's answer to `/systemone`: the key it picked, with each key's probability.
    Decided(String, Vec<(String, f64)>),
}

impl Reply {
    pub(crate) fn text(text: &str) -> Self {
        Self::Text(text.into())
    }

    /// The model calls `name` with `arguments`.
    pub(crate) fn call(name: &str, arguments: Value) -> Self {
        Self::Calls(vec![(name.into(), arguments)])
    }
}

#[derive(Default)]
struct Shared {
    script: VecDeque<Reply>,
    requests: Vec<Value>,
    calls: usize,
    /// The title a title's request is answered with; `None` echoes the task.
    title: Option<String>,
    /// The tasks titles were asked for.
    titled: Vec<String>,
}

/// How the title's request starts (`meta::title::INSTRUCTIONS`).
const TITLE: &str = "Generate a concise, single-line task title";

/// The server; it stops with the test process.
pub(crate) struct Model {
    pub(crate) base: String,
    shared: Arc<Mutex<Shared>>,
}

impl Model {
    /// A model that answers `script` in order, then "Done." to anything more.
    pub(crate) fn start(script: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let shared = Arc::new(Mutex::new(Shared {
            script: script.into(),
            ..Shared::default()
        }));
        let serving = Arc::clone(&shared);
        let server = std::thread::Builder::new().name("model".into());
        server
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let shared = Arc::clone(&serving);
                    let answering = std::thread::Builder::new().name("model-answer".into());
                    answering.spawn(move || answer(stream, &shared)).unwrap();
                }
            })
            .unwrap();
        Self { base, shared }
    }

    /// The same, answering a title's request with `title`.
    pub(crate) fn titling(script: Vec<Reply>, title: &str) -> Self {
        let model = Self::start(script);
        model.shared.lock().unwrap().title = Some(title.into());
        model
    }

    /// The tasks a title was asked for, in order.
    pub(crate) fn titled(&self) -> Vec<String> {
        self.shared.lock().unwrap().titled.clone()
    }

    /// Every request the model received but the titles', as JSON, in order.
    pub(crate) fn requests(&self) -> Vec<Value> {
        self.shared.lock().unwrap().requests.clone()
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "a test's server waits, bounded, for the file the test makes: there is no event to wait on"
)]
fn answer(stream: TcpStream, shared: &Mutex<Shared>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut length = 0;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
    let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    if content(&request["messages"][0]).starts_with(TITLE) {
        let asked = content(&request["messages"][1]);
        let task = asked
            .split_once("User prompt:\n")
            .map_or("", |(_, task)| task);
        let title = {
            let mut shared = shared.lock().unwrap();
            shared.titled.push(task.to_string());
            shared.title.clone().unwrap_or_else(|| task.to_string())
        };
        return respond(
            stream,
            200,
            &completion(Some(Reply::Text(title)), 0, &request),
        );
    }
    let (mut next, n) = {
        let mut shared = shared.lock().unwrap();
        shared.requests.push(request.clone());
        shared.calls += 1;
        (shared.script.pop_front(), shared.calls)
    };
    while let Some(Reply::After(file, reply)) = next {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !file.exists() && std::time::Instant::now() < end {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        next = Some(*reply);
    }
    let reply = {
        match next {
            Some(Reply::Refused(status)) => (
                status,
                json!({"error": {"code": status, "message": "No auth credentials found"}}),
            ),
            Some(Reply::Decided(choice, probabilities)) => {
                (200, decided(&request, &choice, &probabilities))
            }
            reply => (200, completion(reply, n, &request)),
        }
    };
    respond(stream, reply.0, &reply.1);
}

/// A message's text: a string, or the text of its parts (a cache mark makes it parts).
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn content(message: &Value) -> String {
    match &message["content"] {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect(),
        _ => String::new(),
    }
}

fn respond(mut stream: TcpStream, status: u16, body: &Value) {
    let text = body.to_string();
    let _ = write!(
        stream,
        "HTTP/1.1 {status} Status\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{text}",
        text.len()
    );
}

/// The chat completion for `reply` (the `n`th request).
fn completion(reply: Option<Reply>, n: usize, request: &Value) -> Value {
    let model = request["model"].as_str().unwrap_or("test/model");
    let message = match reply.unwrap_or_else(|| Reply::Text("Done.\nRESULT: achieved".into())) {
        Reply::Text(text) => json!({"role": "assistant", "content": text}),
        Reply::Refused(_) | Reply::After(..) | Reply::Decided(..) => {
            unreachable!("answered before")
        }
        Reply::Calls(calls) => {
            let calls: Vec<Value> = calls
                .into_iter()
                .enumerate()
                .map(|(at, (name, arguments))| {
                    json!({"id": format!("call_{n}_{at}"), "type": "function",
                        "function": {"name": name, "arguments": arguments.to_string()}})
                })
                .collect();
            json!({"role": "assistant", "content": null, "tool_calls": calls})
        }
    };
    json!({"id": format!("c{n}"), "model": model, "choices": [{"index": 0, "message": message}],
        "usage": {"prompt_tokens": 1000, "completion_tokens": 20, "cost": 0.0001}})
}

/// `/systemone`'s answer: every question asked gets `choice`, with `probabilities`.
fn decided(request: &Value, choice: &str, probabilities: &[(String, f64)]) -> Value {
    let odds: serde_json::Map<String, Value> = (probabilities.iter())
        .map(|(key, p)| (key.clone(), json!(p)))
        .collect();
    let answers: serde_json::Map<String, Value> = (request["questions"].as_object().unwrap())
        .keys()
        .map(|key| {
            let answer = json!({"type": "choice", "choice": choice, "probabilities": odds,
                "confidence": 0.9});
            (key.clone(), answer)
        })
        .collect();
    json!({"model": request["model"], "answers": answers,
        "usage": {"cost": 0.00002, "input_tokens": 300, "output_tokens": 1}})
}

/// What the model was asked in its `at`th request, as one text: its messages' contents.
pub(crate) fn said(model: &Model, at: usize) -> String {
    model.requests()[at]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["content"].to_string())
        .collect::<Vec<_>>()
        .join("\n")
}
