use std::sync::Arc;

use contracts::catalog::Selection;
use serde_json::json;

use super::*;
use crate::catalog;
use crate::testing::{Scripted, access};

const APP: App = App {
    name: "llx",
    slug: "llx",
    version: "0",
};

/// A store that keeps tokens in memory and counts the saves.
#[derive(Default)]
struct Memory {
    kept: Mutex<Option<Tokens>>,
}

impl Keep for Arc<Memory> {
    fn load(&self) -> Option<Tokens> {
        self.kept.lock().unwrap().clone()
    }

    fn save(&self, tokens: &Tokens) {
        *self.kept.lock().unwrap() = Some(tokens.clone());
    }
}

/// An `Http` the test keeps a handle on.
struct Shared(Arc<Scripted>);

impl Http for Shared {
    fn post(
        &self,
        a: &str,
        h: &[(&str, &str)],
        b: &str,
    ) -> Result<provider_kit::http::Reply, String> {
        self.0.post(a, h, b)
    }
}

fn tokens(expires: u64) -> Tokens {
    Tokens {
        access: access("acct-1", expires),
        refresh: "r1".into(),
        id: None,
        account: "acct-1".into(),
        expires,
    }
}

const LATER: u64 = 9_999_999_999;

fn model(replies: Vec<(u16, String)>, held: Tokens) -> (ChatModel, Arc<Scripted>, Arc<Memory>) {
    let http = Arc::new(Scripted::new(replies));
    let memory = Arc::new(Memory::default());
    let selection = Selection::new(catalog::entry("gpt-6-sol"));
    let model = ChatModel::new(
        APP,
        Box::new(Shared(http.clone())),
        Box::new(memory.clone()),
        held,
        selection,
    );
    (model, http, memory)
}

fn streamed(events: &[Value]) -> String {
    events
        .iter()
        .map(|event| format!("event: x\ndata: {event}\n\n"))
        .collect()
}

fn completed() -> Value {
    json!({"type": "response.completed", "response": {"model": "gpt-6-sol",
        "usage": {"input_tokens": 10, "input_tokens_details": {"cached_tokens": 4}, "output_tokens": 3}}})
}

fn conversation() -> Vec<Message> {
    vec![
        Message::System("rules".into()),
        Message::User("goal".into()),
        Message::Assistant {
            text: Some("looking".into()),
            calls: vec![ToolCall {
                id: "c1".into(),
                name: "read".into(),
                arguments: json!({"path": "a"}),
            }],
        },
        Message::ToolResult {
            id: "c1".into(),
            content: "missing".into(),
            failed: true,
        },
    ]
}

#[test]
fn a_conversation_goes_out_as_input_items_with_the_level_chosen() {
    let spec = ToolSpec {
        name: "read".into(),
        description: "Read".into(),
        parameters: json!({"type": "object"}),
    };
    let conversation = conversation();
    let request = Request {
        conversation: &conversation,
        tools: std::slice::from_ref(&spec),
        may_call: true,
    };
    let selection = Selection::new(catalog::entry("gpt-6-sol"))
        .with_effort(Effort::High)
        .unwrap();
    let body = body(&selection, "s1", request);
    assert_eq!(body["model"], "gpt-6-sol");
    assert_eq!(
        (body["store"].clone(), body["stream"].clone()),
        (json!(false), json!(true))
    );
    assert_eq!(body["reasoning"], json!({"effort": "high"}));
    assert_eq!(body["prompt_cache_key"], "s1");
    let input = body["input"].as_array().unwrap();
    assert_eq!(input[0]["role"], "developer");
    assert_eq!(
        input[1]["content"][0],
        json!({"type": "input_text", "text": "goal"})
    );
    assert_eq!(input[2]["content"][0]["type"], "output_text");
    assert_eq!(
        input[3],
        json!({"type": "function_call", "call_id": "c1", "name": "read",
        "arguments": "{\"path\":\"a\"}"})
    );
    assert_eq!(
        input[4],
        json!({"type": "function_call_output", "call_id": "c1",
        "output": "Error: missing"})
    );
    assert_eq!(body["tools"][0]["name"], "read");
    assert_eq!(
        (
            body["tool_choice"].clone(),
            body["parallel_tool_calls"].clone()
        ),
        (json!("auto"), json!(false))
    );
}

#[test]
fn a_model_with_no_level_is_sent_none() {
    let request = Request {
        conversation: &[],
        tools: &[],
        may_call: false,
    };
    let body = body(&Selection::new(catalog::entry("gpt-next")), "s", request);
    assert!(body.get("reasoning").is_none() && body.get("tools").is_none());
}

#[test]
fn a_streamed_reply_becomes_text_calls_and_usage() {
    let reply = streamed(&[
        json!({"type": "response.output_text.delta", "delta": "ignored"}),
        json!({"type": "response.output_item.done", "item": {"type": "reasoning"}}),
        json!({"type": "response.output_item.done", "item": {"type": "message",
            "content": [{"type": "output_text", "text": " Done. "}]}}),
        json!({"type": "response.output_item.done", "item": {"type": "function_call",
            "call_id": "c2", "name": "edit", "arguments": "{\"path\": \"a"}}),
        completed(),
    ]);
    let completion = read(&reply, "fallback").unwrap();
    assert_eq!(completion.text.as_deref(), Some("Done."));
    assert_eq!(completion.calls[0].id, "c2");
    assert_eq!(
        completion.calls[0].arguments,
        json!("{\"path\": \"a"),
        "cut arguments stay text"
    );
    let usage = &completion.usage;
    assert_eq!(
        (usage.input_tokens, usage.cached_tokens, usage.output_tokens),
        (10, 4, 3)
    );
    assert_eq!(usage.model, "gpt-6-sol");
}

#[test]
fn a_failed_or_cut_stream_is_an_error() {
    let failed = streamed(&[
        json!({"type": "response.failed", "response": {"error": {"message": "overloaded"}}}),
    ]);
    assert!(
        matches!(read(&failed, "m"), Err(ChatError::Unavailable(m)) if m.contains("overloaded"))
    );
    let cut = streamed(&[json!({"type": "response.created"})]);
    assert!(matches!(read(&cut, "m"), Err(ChatError::Invalid(_))));
}

#[test]
fn a_request_names_the_account_and_the_token() {
    let (model, http, _) = model(vec![(200, streamed(&[completed()]))], tokens(LATER));
    let conversation = conversation();
    let request = Request {
        conversation: &conversation,
        tools: &[],
        may_call: false,
    };
    model.complete(request).unwrap();
    let sent = &http.sent()[0];
    assert_eq!(
        sent.address,
        "https://chatgpt.com/backend-api/codex/responses"
    );
    assert_eq!(sent.header("chatgpt-account-id"), Some("acct-1"));
    assert_eq!(
        sent.header("authorization"),
        Some(format!("Bearer {}", access("acct-1", LATER)).as_str())
    );
    assert_eq!(sent.header("originator"), Some("llx"));
    assert_eq!(sent.header("accept"), Some("text/event-stream"));
}

#[test]
fn a_token_about_to_lapse_is_refreshed_and_kept_first() {
    let refreshed = json!({"access_token": access("acct-1", LATER), "refresh_token": "r2"});
    let replies = vec![
        (200, refreshed.to_string()),
        (200, streamed(&[completed()])),
    ];
    let (model, http, memory) = model(replies, tokens(1));
    let request = Request {
        conversation: &[],
        tools: &[],
        may_call: false,
    };
    model.complete(request).unwrap();
    let sent = http.sent();
    assert_eq!(sent[0].address, "https://auth.openai.com/oauth/token");
    assert_eq!(
        sent[1].header("authorization"),
        Some(format!("Bearer {}", access("acct-1", LATER)).as_str())
    );
    assert_eq!(memory.load().map(|t| t.refresh), Some("r2".into()));
}

#[test]
fn a_refused_token_is_refreshed_once_then_the_refusal_is_said() {
    let refreshed = json!({"access_token": access("acct-1", LATER), "refresh_token": "r2"});
    let replies = vec![
        (401, "{}".into()),
        (200, refreshed.to_string()),
        (401, r#"{"detail": "nope"}"#.into()),
    ];
    let (model, http, _) = model(replies, tokens(LATER));
    let request = Request {
        conversation: &[],
        tools: &[],
        may_call: false,
    };
    let refused = model.complete(request).unwrap_err().to_string();
    assert!(
        refused.contains("nope") && refused.contains("/login"),
        "{refused}"
    );
    assert_eq!(http.sent().len(), 3, "one refresh, no loop");
}

#[test]
fn a_usage_limit_says_so() {
    let (model, _, _) = model(
        vec![(429, r#"{"error": {"message": "try later"}}"#.into())],
        tokens(LATER),
    );
    let request = Request {
        conversation: &[],
        tools: &[],
        may_call: false,
    };
    let refused = model.complete(request).unwrap_err().to_string();
    assert!(refused.contains("usage limit") && refused.contains("try later"));
}
