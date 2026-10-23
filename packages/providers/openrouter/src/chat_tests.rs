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
    assert_eq!(
        messages[1]["content"][0]["cache_control"]["type"], "ephemeral",
        "the context after the system prompt is marked"
    );
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

fn selection(efforts: Vec<Effort>, effort: Effort) -> Selection {
    let model = contracts::catalog::ModelEntry {
        id: "lab/model".into(),
        name: "Model".into(),
        default: efforts.first().copied().unwrap_or(Effort::Off),
        efforts,
        context: 100_000,
    };
    Selection::new(model).with_effort(effort).unwrap()
}

fn request_body(openrouter: bool, selection: &Selection, may_call: bool) -> Value {
    let conversation = [
        Message::System("rules".into()),
        Message::User("context".into()),
        Message::User("goal".into()),
    ];
    let tools = [ToolSpec {
        name: "read".into(),
        description: "Read.".into(),
        parameters: json!({"type": "object"}),
    }];
    let request = Request {
        conversation: &conversation,
        tools: &tools,
        may_call,
    };
    body(openrouter, selection, &request)
}

#[test]
fn openrouter_is_sent_its_reasoning_object_the_usage_report_and_cache_marks() {
    let levels = vec![Effort::Off, Effort::Low, Effort::High];
    let sent = request_body(true, &selection(levels.clone(), Effort::High), true);
    assert_eq!(sent["reasoning"], json!({"effort": "high"}));
    assert_eq!(sent["usage"], json!({"include": true}));
    assert_eq!(
        sent["messages"][0]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert!(sent.get("reasoning_effort").is_none());
    assert_eq!(sent["tool_choice"], "auto");
    assert_eq!(sent["parallel_tool_calls"], true);
    let off = request_body(true, &selection(levels, Effort::Off), false);
    assert_eq!(off["reasoning"], json!({"enabled": false}));
    assert_eq!(off["tool_choice"], "none", "the wrap-up turn calls nothing");
}

#[test]
fn a_model_with_no_levels_is_sent_no_reasoning_field_on_openrouter_either() {
    let sent = request_body(true, &selection(vec![], Effort::Off), true);
    assert!(sent.get("reasoning").is_none(), "{sent}");
    let generic = request_body(false, &selection(vec![], Effort::Off), true);
    assert!(generic.get("reasoning_effort").is_none());
}

#[test]
fn another_host_is_sent_reasoning_effort_by_name_and_no_openrouter_fields() {
    let levels = vec![Effort::Off, Effort::Low, Effort::High];
    let sent = request_body(false, &selection(levels.clone(), Effort::Low), true);
    assert_eq!(sent["reasoning_effort"], "low");
    assert!(sent.get("reasoning").is_none() && sent.get("usage").is_none());
    assert_eq!(sent["messages"][0]["content"], "rules", "no cache marks");
    let off = request_body(false, &selection(levels, Effort::Off), true);
    assert_eq!(off["reasoning_effort"], "none");
}
