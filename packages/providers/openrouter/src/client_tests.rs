use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use contracts::catalog::{Effort, ModelEntry, Selection};
use contracts::chat::{Chat, Message, Request};
use serde_json::json;

use super::*;

const APP: crate::App = crate::App {
    name: "llx",
    slug: "llx",
    version: "0",
};

/// A server on 127.0.0.1 that answers each request with the next of `replies` (status, body),
/// or never when they run out; it keeps every request body, and counts the connections.
fn serve(replies: Vec<(u16, &'static str)>) -> (String, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(vec![]));
    let kept = seen.clone();
    std::thread::spawn(move || {
        let mut replies = replies.into_iter();
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            kept.lock()
                .unwrap()
                .push(serde_json::from_slice(&body).unwrap_or(Value::Null));
            let Some((status, text)) = replies.next() else {
                // Hold the connection open and say nothing.
                std::thread::spawn(move || {
                    let _keep = stream;
                    std::thread::sleep(Duration::from_secs(30));
                });
                continue;
            };
            let mut stream = stream;
            let _ = write!(
                stream,
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                text.len()
            );
        }
    });
    (base, seen)
}

#[test]
fn an_overloaded_answer_is_retried_and_a_refused_key_is_not() {
    let (base, seen) = serve(vec![(503, "{}"), (200, r#"{"ok":1}"#)]);
    let client = OpenRouter::at(&base, "k".into(), APP, "Lab");
    assert_eq!(client.post("/x", &json!({})).unwrap().0, json!({"ok": 1}));
    assert_eq!(seen.lock().unwrap().len(), 2);

    let (base, seen) = serve(vec![(401, r#"{"error":{"message":"User not found."}}"#)]);
    let error = OpenRouter::at(&base, "k".into(), APP, "Lab")
        .post("/x", &json!({}))
        .unwrap_err();
    assert_eq!(error, ApiError::Auth("User not found.".into()));
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert_eq!(
        format!("Lab {error}"),
        "Lab refused the key: User not found.; type /login, or check the key's variable"
    );
}

#[test]
fn a_request_that_times_out_is_not_sent_again() {
    // A reply being written is billed: sending it again pays for it twice.
    let (base, seen) = serve(vec![]);
    let client = OpenRouter {
        agent: provider_kit::http::agent(APP, Duration::from_millis(500)),
        ..OpenRouter::at(&base, "k".into(), APP, "Lab")
    };
    let error = client.post("/x", &json!({})).unwrap_err();
    assert!(matches!(error, ApiError::Network(_)), "{error:?}");
    assert_eq!(seen.lock().unwrap().len(), 1, "sent once");
}

#[test]
fn openrouter_at_another_address_is_still_sent_openrouters_fields() {
    // The test servers stand in for OpenRouter at a local address: they must see its wire.
    let (base, seen) = serve(vec![(200, r#"{"choices":[{"message":{"content":"hi"}}]}"#)]);
    // SAFETY: no other test in this crate reads the variable.
    unsafe { std::env::set_var("OPENROUTER_BASE_URL", &base) };
    let client = OpenRouter::new("k".into(), APP);
    assert!(client.is_openrouter());
    assert!(!OpenRouter::at(&base, "k".into(), APP, "Lab").is_openrouter());
    let model = ModelEntry {
        id: "lab/model".into(),
        name: "Model".into(),
        efforts: vec![Effort::Low, Effort::High],
        default: Effort::High,
        context: 1000,
    };
    let chat = crate::chat::ChatModel {
        client,
        selection: Selection::new(model),
    };
    let conversation = [Message::User("goal".into())];
    let request = Request {
        conversation: &conversation,
        tools: &[],
        may_call: true,
    };
    assert_eq!(chat.complete(request).unwrap().text.as_deref(), Some("hi"));
    let sent = seen.lock().unwrap()[0].clone();
    assert_eq!(sent["reasoning"], json!({"effort": "high"}));
    assert_eq!(sent["usage"], json!({"include": true}));
}

#[test]
fn a_decision_is_quick_and_one_that_times_out_is_asked_once_more() {
    // A decision is short and cheap: one that hangs is better asked again than waited on.
    let deciding = OpenRouter::deciding("k".into(), APP);
    let global = deciding.agent.config().timeouts().global;
    assert_eq!(global, Some(Duration::from_secs(90)));
    assert!(deciding.is_openrouter());
    let (base, seen) = serve(vec![]);
    let client = OpenRouter {
        agent: provider_kit::http::agent(APP, Duration::from_millis(300)),
        base,
        ..deciding
    };
    let error = client.post("/x", &json!({})).unwrap_err();
    assert!(matches!(error, ApiError::Network(_)), "{error:?}");
    assert_eq!(seen.lock().unwrap().len(), 2, "sent twice, not more");
}
