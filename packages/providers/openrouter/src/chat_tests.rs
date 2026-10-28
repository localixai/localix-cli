use super::*;
use contracts::tool::{Grammar, Input};

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
                carried: vec![],
            },
            Message::ToolResult {
                id: "c1".into(),
                content: "page".into(),
                failed: false,
            },
        ]
        .iter()
        .map(|message| wire(message, "m", &[]))
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
        .map(|message| wire(message, "m", &[]))
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
    selection_of("lab/model", efforts, effort)
}

fn selection_of(id: &str, efforts: Vec<Effort>, effort: Effort) -> Selection {
    let model = contracts::catalog::ModelEntry {
        id: id.into(),
        name: "Model".into(),
        default: efforts.first().copied().unwrap_or(Effort::Off),
        efforts,
        context: 100_000,
    };
    Selection::new(model).with_effort(effort).unwrap()
}

fn request_body(wire: Wire, selection: &Selection, may_call: bool) -> Value {
    let conversation = [
        Message::System("rules".into()),
        Message::User("context".into()),
        Message::User("goal".into()),
    ];
    let tools = [ToolSpec {
        name: "read".into(),
        description: "Read.".into(),
        input: Input::Json(json!({"type": "object"})),
    }];
    let request = Request {
        conversation: &conversation,
        tools: &tools,
        may_call,
    };
    body(wire, selection, &request)
}

#[test]
fn openrouter_is_sent_its_reasoning_object_the_usage_report_and_cache_marks() {
    let levels = vec![Effort::Off, Effort::Low, Effort::High];
    let sent = request_body(
        Wire::OpenRouter,
        &selection_of("anthropic/claude-x", levels.clone(), Effort::High),
        true,
    );
    assert_eq!(sent["reasoning"], json!({"effort": "high"}));
    assert_eq!(sent["usage"], json!({"include": true}));
    assert_eq!(
        sent["messages"][0]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert!(sent.get("reasoning_effort").is_none());
    assert_eq!(sent["tool_choice"], "auto");
    assert_eq!(sent["parallel_tool_calls"], true);
    let off = request_body(Wire::OpenRouter, &selection(levels, Effort::Off), false);
    assert_eq!(off["reasoning"], json!({"enabled": false}));
    assert_eq!(off["tool_choice"], "none", "the wrap-up turn calls nothing");
}

#[test]
fn a_model_with_no_levels_is_sent_no_reasoning_field_on_openrouter_either() {
    let sent = request_body(Wire::OpenRouter, &selection(vec![], Effort::Off), true);
    assert!(sent.get("reasoning").is_none(), "{sent}");
    let generic = request_body(Wire::Plain, &selection(vec![], Effort::Off), true);
    assert!(generic.get("reasoning_effort").is_none());
}

#[test]
fn another_host_is_sent_reasoning_effort_by_name_and_no_openrouter_fields() {
    let levels = vec![Effort::Off, Effort::Low, Effort::High];
    let sent = request_body(Wire::Plain, &selection(levels.clone(), Effort::Low), true);
    assert_eq!(sent["reasoning_effort"], "low");
    assert!(sent.get("reasoning").is_none() && sent.get("usage").is_none());
    assert_eq!(sent["messages"][0]["content"], "rules", "no cache marks");
    let off = request_body(Wire::Plain, &selection(levels, Effort::Off), true);
    assert_eq!(off["reasoning_effort"], "none");
}

#[test]
fn openai_itself_is_sent_one_cache_key_for_every_session_with_the_same_start_and_others_none() {
    let tools = [ToolSpec {
        name: "read".into(),
        description: "Read.".into(),
        input: Input::Json(json!({"type": "object"})),
    }];
    let key = |wire: Wire, tools: &[ToolSpec], goal: &str| {
        let conversation = [Message::System("rules".into()), Message::User(goal.into())];
        let request = Request {
            conversation: &conversation,
            tools,
            may_call: true,
        };
        body(wire, &selection(vec![], Effort::Off), &request)["prompt_cache_key"].clone()
    };
    // Two sessions, each its own goal: one key, so OpenAI sends both to where the start is cached.
    let first = key(Wire::OpenAi, &tools, "one goal");
    assert!(first.is_string(), "{first}");
    assert_eq!(first, key(Wire::OpenAi, &tools, "another goal"));
    assert_ne!(first, key(Wire::OpenAi, &[], "one goal"));
    // A host that may refuse a field it does not know is sent none; OpenRouter has its marks.
    assert_eq!(key(Wire::Plain, &tools, "one goal"), Value::Null);
    assert_eq!(key(Wire::OpenRouter, &tools, "one goal"), Value::Null);
}

#[test]
fn the_models_reasoning_goes_back_to_it_with_the_next_request() {
    // A model that thinks between its tool calls needs its reasoning back, whole: OpenRouter's
    // `reasoning_details` (signed or sealed parts), else the text in the field it came in.
    let details = json!([{"type": "reasoning.encrypted", "data": "sealed", "id": "r1",
        "format": "anthropic-claude-v1", "index": 0}]);
    let reply = json!({"model": "m", "choices": [{"message": {"content": null,
        "reasoning": "thinking", "reasoning_details": details,
        "tool_calls": [{"id": "c1", "type": "function",
            "function": {"name": "read", "arguments": "{}"}}]}}]});
    let completion = read(&reply, "m", 0).unwrap();
    let sent = wire(&completion.message(), "m", &[]);
    assert_eq!(sent["reasoning_details"], details);
    assert!(
        sent.get("reasoning").is_none(),
        "the details stand for the text"
    );
    assert_eq!(sent["tool_calls"][0]["id"], "c1");

    // A host that says it in `reasoning_content` (DeepSeek's own API) is answered in it.
    let reply = json!({"model": "m", "choices": [{"message": {"content": "Done.",
        "reasoning_content": "step by step"}}]});
    let sent = wire(&read(&reply, "m", 0).unwrap().message(), "m", &[]);
    assert_eq!(sent["reasoning_content"], "step by step");

    // Another model (after /model) is sent none of it.
    let other = wire(&completion.message(), "other", &[]);
    assert!(other.get("reasoning_details").is_none() && other.get("reasoning").is_none());
}

#[test]
fn cache_marks_go_only_to_the_models_that_take_explicit_breakpoints() {
    let marked = |id: &str| {
        let sent = request_body(
            Wire::OpenRouter,
            &selection_of(id, vec![], Effort::Off),
            true,
        );
        sent.to_string().contains("cache_control")
    };
    for id in [
        "anthropic/claude-x",
        "google/gemini-x",
        "qwen/qwen3-coder-plus",
    ] {
        assert!(marked(id), "{id} needs explicit breakpoints");
    }
    for id in [
        "openai/gpt-x",
        "deepseek/deepseek-x",
        "x-ai/grok-x",
        "moonshotai/kimi-x",
        "google/gemma-x",
        "lab/model",
    ] {
        assert!(!marked(id), "{id} caches on its own: no marks");
    }
    let sent = request_body(
        Wire::OpenRouter,
        &selection_of("openai/gpt-x", vec![], Effort::Off),
        true,
    );
    assert_eq!(
        sent["messages"][0]["content"], "rules",
        "the content stays a plain string"
    );
}

fn script() -> ToolSpec {
    ToolSpec {
        name: "exec".into(),
        description: "Run a script.".into(),
        input: Input::Text(Grammar::Lark("start: /.+/".into())),
    }
}

#[test]
fn a_script_is_offered_as_a_function_with_one_string_argument() {
    let conversation = [Message::User("goal".into())];
    let tools = [script()];
    let request = Request {
        conversation: &conversation,
        tools: &tools,
        may_call: true,
    };
    let body = body(Wire::OpenRouter, &selection(vec![], Effort::Off), &request);
    let function = &body["tools"][0]["function"];
    assert_eq!(function["name"], "exec");
    assert_eq!(function["parameters"]["required"], json!(["source"]));
    assert_eq!(
        function["parameters"]["properties"]["source"]["type"],
        "string"
    );
}

#[test]
fn a_scripts_call_is_its_text_coming_in_and_its_one_argument_going_back() {
    let wired = json!({"id": "c1", "function": {"name": "exec",
        "arguments": "{\"source\":\"return 1\"}"}});
    let call = read_call(&wired).unwrap();
    // The reader keeps what the model wrote; the port takes the text out for the agent.
    let tools = [script()];
    let call = incoming(call, &tools);
    assert_eq!(call.arguments, json!("return 1"));

    let sent = wire(
        &Message::Assistant {
            text: None,
            calls: vec![call],
            carried: vec![],
        },
        "m",
        &tools,
    );
    let arguments = sent["tool_calls"][0]["function"]["arguments"]
        .as_str()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(arguments).unwrap(),
        json!({"source": "return 1"})
    );
}
