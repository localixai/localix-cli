use client::CallError;
use protocol::{Done, Picture};
use serde_json::{Value, json};

use super::*;

#[test]
fn a_request_a_notification_and_a_response_are_told_apart() {
    let request = read(r#"{"jsonrpc":"2.0","id":"a1","method":"tools/list"}"#);
    assert_eq!(
        request,
        Message::Request {
            id: json!("a1"),
            method: "tools/list".into(),
            params: Value::Null,
        }
    );
    let cancelled =
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":3}}"#;
    assert!(matches!(read(cancelled), Message::Notification { .. }));
    assert_eq!(
        read(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#),
        Message::Response
    );
    assert_eq!(read("not json"), Message::Unreadable);
    assert_eq!(read("[1, 2]"), Message::Unreadable);
}

#[test]
fn initialize_answers_with_the_version_asked_when_spoken_else_the_newest() {
    let asked = initialized(&json!({"protocolVersion": "2025-06-18"}), "0.9.0");
    assert_eq!(asked["protocolVersion"], "2025-06-18");
    assert_eq!(
        asked["serverInfo"],
        json!({"name": "llx", "version": "0.9.0"})
    );
    assert_eq!(
        asked["capabilities"]["tools"],
        json!({"listChanged": false})
    );
    let unknown = initialized(&json!({"protocolVersion": "1999-01-01"}), "0.9.0");
    assert_eq!(unknown["protocolVersion"], "2025-11-25");
}

#[test]
fn a_call_reads_as_its_report_then_its_state_with_its_picture() {
    let done = Done::Tool {
        report: "Opened w1.".into(),
        state: Some("# Example".into()),
        image: Some(Picture {
            media_type: "image/jpeg".into(),
            data: "/9j/".into(),
        }),
    };
    let read = called(Ok(done));
    assert_eq!(read["isError"], false);
    assert_eq!(
        read["content"],
        json!([
            {"type": "text", "text": "Opened w1.\n\n# Example"},
            {"type": "image", "data": "/9j/", "mimeType": "image/jpeg"},
        ])
    );
}

#[test]
fn a_failed_call_is_a_result_the_model_reads_not_a_protocol_error() {
    let read = called(Err(CallError::Failed("there is no tab w9".into())));
    assert_eq!(read["isError"], true);
    assert_eq!(read["content"][0]["text"], "there is no tab w9");
    let gone = called(Err(CallError::Gone));
    assert_eq!(gone["isError"], true);
}
