//! A scripted model behind a real HTTP server:
//! the binary under test calls it as it calls OpenRouter (`OPENROUTER_BASE_URL`), it answers each
//! chat completion from a script, and it keeps every request for the test to read. No network.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

/// One answer of the model: text, tool calls, or an HTTP error with its status.
pub enum Reply {
    Text(String),
    Calls(Vec<(String, Value)>),
    Refused(u16),
    /// The reply given once the file exists: a lane's request waits while another works.
    After(std::path::PathBuf, Box<Reply>),
}

impl Reply {
    pub fn text(text: &str) -> Self {
        Self::Text(text.into())
    }

    pub fn call(name: &str, arguments: Value) -> Self {
        Self::Calls(vec![(name.into(), arguments)])
    }
}

#[derive(Default)]
struct Shared {
    script: VecDeque<Reply>,
    requests: Vec<Value>,
    calls: usize,
}

/// The server; it stops with the test process.
pub struct Model {
    pub base: String,
    shared: Arc<Mutex<Shared>>,
}

impl Model {
    /// A model that answers `script` in order, then "Done." to anything more.
    pub fn start(script: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let shared = Arc::new(Mutex::new(Shared {
            script: script.into(),
            ..Shared::default()
        }));
        let serving = shared.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let shared = serving.clone();
                std::thread::spawn(move || answer(stream, &shared));
            }
        });
        Self { base, shared }
    }

    /// Every request the model received, as JSON, in order.
    pub fn requests(&self) -> Vec<Value> {
        self.shared.lock().unwrap().requests.clone()
    }
}

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
            reply => (200, completion(reply, n, &request)),
        }
    };
    let (status, text) = (reply.0, reply.1.to_string());
    let mut stream = stream;
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
    let message = match reply.unwrap_or(Reply::Text("Done.\nRESULT: achieved".into())) {
        Reply::Text(text) => json!({"role": "assistant", "content": text}),
        Reply::Refused(_) | Reply::After(..) => unreachable!("answered before"),
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

/// What the model was asked in its `at`th request, as one text: its messages' contents.
pub fn said(model: &Model, at: usize) -> String {
    model.requests()[at]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["content"].to_string())
        .collect::<Vec<_>>()
        .join("\n")
}
