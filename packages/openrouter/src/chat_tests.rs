use super::*;

#[test]
fn a_conversation_goes_out_in_the_wire_format_with_cache_marks() {
    let call = ToolCall {
        id: "c1".into(),
        name: "browser".into(),
        arguments: json!({"url": "https://example.com"}),
    };
    let messages = cached(
        [
            Message::System("rules".into()),
            Message::User("goal".into()),
            Message::Assistant {
                text: None,
                calls: vec![call],
            },
            Message::ToolResult {
                id: "c1".into(),
                content: "page".into(),
                failed: false,
            },
        ]
        .iter()
        .map(wire)
        .collect(),
    );
    assert_eq!(
        messages[0]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert_eq!(messages[1]["content"], "goal", "only the ends are marked");
    assert_eq!(
        messages[2]["tool_calls"][0]["function"]["arguments"],
        r#"{"url":"https://example.com"}"#
    );
    assert_eq!(messages[3]["tool_call_id"], "c1");
    assert_eq!(messages[3]["content"][0]["text"], "page");
}

#[test]
fn a_tool_call_with_unreadable_arguments_arrives_as_their_text() {
    // A reply cut short leaves `{"path": "a.html", "new_string": "<h` behind: it must not
    // become an empty call that fails as "missing field path".
    let call = read_call(
        &json!({"id": "c2", "function": {"name": "edit", "arguments": "{\"path\": \"a"}}),
    );
    assert_eq!(call.map(|c| c.arguments), Some(json!("{\"path\": \"a")));
}
