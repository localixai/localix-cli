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

#[test]
fn a_picture_goes_out_as_a_user_message_with_an_image_part_and_the_mark() {
    let image = Image {
        media_type: "image/jpeg".into(),
        data: "/9j/AA==".into(),
        width: 2,
        height: 1,
    };
    let messages = cached(
        [
            Message::System("rules".into()),
            Message::Image {
                caption: "The screenshot".into(),
                image,
            },
        ]
        .iter()
        .map(wire)
        .collect(),
    );
    let picture = &messages[1];
    assert_eq!(picture["role"], "user");
    assert_eq!(picture["content"][0]["text"], "The screenshot");
    assert_eq!(picture["content"][1]["type"], "image_url");
    assert_eq!(
        picture["content"][1]["image_url"]["url"],
        "data:image/jpeg;base64,/9j/AA=="
    );
    assert_eq!(
        picture["content"][1]["cache_control"]["type"], "ephemeral",
        "the newest message keeps its cache mark"
    );
}

#[test]
fn off_turns_thinking_off_and_a_level_is_asked_by_name() {
    assert_eq!(reasoning(Effort::Off), json!({"enabled": false}));
    assert_eq!(reasoning(Effort::XHigh), json!({"effort": "xhigh"}));
}
