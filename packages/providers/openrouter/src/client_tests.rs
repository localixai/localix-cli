use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use contracts::catalog::{Effort, ModelEntry, Selection};
use contracts::chat::{Chat, Message, Request};
use provider_kit::http::Reply;
use serde_json::json;

use super::*;

const APP: App = App {
    name: "llx",
    slug: "llx",
    version: "0",
};

/// An `Http` that gives each request the next of its answers (a reply, or why none came), and
/// keeps every request body.
struct Scripted {
    answers: Mutex<Vec<Result<Reply, HttpError>>>,
    sent: Arc<Mutex<Vec<Value>>>,
}

impl Http for Scripted {
    #[expect(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn post(
        &self,
        _: &str,
        _: &[(&str, &str)],
        body: &str,
        _: Option<Instant>,
    ) -> Result<Reply, HttpError> {
        let body = serde_json::from_str(body).unwrap_or(Value::Null);
        self.sent.lock().unwrap().push(body);
        let next = self.answers.lock().unwrap().pop();
        next.unwrap_or_else(|| Err(HttpError::Unanswered("nothing scripted".into())))
    }
}

/// `client` on a scripted `Http` answering `answers` in order; the bodies it was sent.
fn scripted(
    client: OpenRouter,
    answers: Vec<Result<(u16, &str), HttpError>>,
) -> (OpenRouter, Arc<Mutex<Vec<Value>>>) {
    let answers = (answers.into_iter().rev())
        .map(|answer| {
            answer.map(|(status, body)| Reply {
                status,
                headers: vec![],
                body: body.into(),
            })
        })
        .collect();
    let sent = Arc::new(Mutex::new(vec![]));
    let http = Scripted {
        answers: Mutex::new(answers),
        sent: Arc::clone(&sent),
    };
    let client = OpenRouter {
        http: Box::new(http),
        ..client
    };
    (client, sent)
}

fn lab() -> OpenRouter {
    OpenRouter::at("http://lab.test", "k".into(), APP, "Lab")
}

#[test]
fn an_overloaded_answer_is_retried_and_a_refused_key_is_not() {
    let (client, sent) = scripted(lab(), vec![Ok((503, "{}")), Ok((200, r#"{"ok":1}"#))]);
    assert_eq!(
        client.post("/x", &json!({}), None).unwrap().0,
        json!({"ok": 1})
    );
    assert_eq!(sent.lock().unwrap().len(), 2);

    let refused = r#"{"error":{"message":"User not found."}}"#;
    let (client, sent) = scripted(lab(), vec![Ok((401, refused))]);
    let error = client.post("/x", &json!({}), None).unwrap_err();
    assert_eq!(error, ApiError::Auth("User not found.".into()));
    assert_eq!(sent.lock().unwrap().len(), 1);
    assert_eq!(
        format!("Lab {error}"),
        "Lab refused the key: User not found.; type /login, or check the key's variable"
    );
}

#[test]
fn a_request_sent_and_not_answered_is_not_sent_again_and_one_not_sent_is() {
    // A reply being written is billed: sending it again pays for it twice.
    let unanswered = HttpError::Unanswered("timed out".into());
    let (client, sent) = scripted(lab(), vec![Err(unanswered)]);
    let error = client.post("/x", &json!({}), None).unwrap_err();
    assert_eq!(error, ApiError::Network("timed out".into()));
    assert_eq!(sent.lock().unwrap().len(), 1, "sent once");
    let unsent = HttpError::Unsent("connection refused".into());
    let (client, sent) = scripted(lab(), vec![Err(unsent), Ok((200, "{}"))]);
    assert_eq!(client.post("/x", &json!({}), None).unwrap().0, json!({}));
    assert_eq!(sent.lock().unwrap().len(), 2, "sent again");
}

#[test]
fn a_decision_not_answered_is_asked_once_more() {
    // A decision is short and cheap: one that hangs is better asked again than waited on.
    let deciding = OpenRouter::new("k".into(), APP).deciding();
    assert_eq!(deciding.wire(), Wire::OpenRouter);
    let unanswered = || Err(HttpError::Unanswered("timed out".into()));
    let (client, sent) = scripted(deciding, vec![unanswered(), unanswered(), unanswered()]);
    let error = client.post("/x", &json!({}), None).unwrap_err();
    assert!(matches!(error, ApiError::Network(_)), "{error:?}");
    assert_eq!(sent.lock().unwrap().len(), 2, "sent twice, not more");
}

#[test]
fn openrouter_at_another_address_is_still_sent_openrouters_fields() {
    // A server standing in for OpenRouter at a local address must see its wire.
    let reply = r#"{"choices":[{"message":{"content":"hi"}}]}"#;
    let local = OpenRouter::standing_in("http://127.0.0.1:1", "k".into(), APP);
    assert_eq!(local.wire(), Wire::OpenRouter);
    assert_eq!(lab().wire(), Wire::Plain);
    let openai = OpenRouter::at("https://api.openai.com/v1/", "k".into(), APP, "OpenAI");
    assert_eq!(openai.wire(), Wire::OpenAi);
    let (client, sent) = scripted(local, vec![Ok((200, reply))]);
    let model = ModelEntry {
        id: "lab/model".into(),
        name: "Model".into(),
        efforts: vec![Effort::Low, Effort::High],
        default: Effort::High,
        context: 1000,
        marks: false,
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
        deadline: None,
    };
    assert_eq!(chat.complete(request).unwrap().text.as_deref(), Some("hi"));
    let sent = sent.lock().unwrap()[0].clone();
    assert_eq!(sent["reasoning"], json!({"effort": "high"}));
    assert_eq!(sent["usage"], json!({"include": true}));
}

/// A server whose one answer promises 100 bytes, sends a few, and closes the connection.
fn cut_short() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = std::thread::Builder::new().name("llx-test-cut".into());
    server
        .spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 2 {
                line.clear();
            }
            let mut stream = stream;
            let head = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 100";
            let _ = write!(stream, "{head}\r\n\r\n{{\"choices\"");
        })
        .unwrap();
    base
}

#[test]
fn an_answer_cut_short_is_said_so_not_taken_for_one_that_is_not_json() {
    let client = OpenRouter::at(&cut_short(), "k".into(), APP, "Lab");
    let error = client.post("/x", &json!({}), None).unwrap_err();
    let ApiError::Network(why) = &error else {
        panic!("{error:?}");
    };
    assert!(why.contains("answer"), "{why}");
}
