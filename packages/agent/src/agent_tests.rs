use contracts::chat::{Completion, Message, Usage};
use contracts::tool::{Tool, ToolError};
use serde_json::{Value, json};

use super::*;
use crate::fakes::Silent;
use crate::fakes::{Echo, Recorder, Scripted, calls, ok, says};

fn agent<'a>(
    chat: &'a Scripted,
    tools: &'a [&'a dyn Tool],
    schema: Option<&'a Value>,
    observer: &'a dyn Observer,
) -> Agent<'a> {
    Agent {
        chat,
        tools,
        context: "",
        max_turns: 5,
        observer,
        schema,
        inbox: None,
        window: 0,
        strategy: &crate::fakes::Keep,
    }
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn results(run: &Run) -> Vec<(String, bool)> {
    run.conversation
        .messages()
        .iter()
        .filter_map(|message| match message {
            Message::ToolResult {
                content, failed, ..
            } => Some((content.clone(), *failed)),
            _ => None,
        })
        .collect()
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn users(run: &Run) -> Vec<String> {
    run.conversation
        .messages()
        .iter()
        .filter_map(|message| match message {
            Message::User(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_reply_without_a_call_is_the_answer_and_its_result_line_the_status() {
    let chat = Scripted::new(vec![
        calls("echo", json!({})),
        Completion {
            usage: Usage {
                output_tokens: 7,
                ..Usage::default()
            },
            ..says("Price is 93 EUR.\nRESULT: achieved")
        },
    ]);
    let echo = Echo::new(vec![ok("opened")]);
    let recorder = Recorder::default();
    let run = agent(&chat, &[&echo], None, &recorder).run("Find the price");
    assert_eq!(run.status, Status::Achieved);
    assert_eq!(run.answer, "Price is 93 EUR.");
    assert_eq!(results(&run), [("opened\n\nstate".to_string(), false)]);
    assert_eq!(run.spend.calls, 2);
    assert_eq!(
        run.spend.output_tokens, 7,
        "the turns' output tokens add up"
    );
    assert_eq!(
        recorder.events(),
        ["turn 0", "call echo", "ok echo: opened", "turn 7"],
        "each turn is reported with what it took"
    );
}

#[test]
fn failed_calls_go_back_to_the_model_and_too_many_ask_for_an_answer() {
    let failed = || Err(ToolError::Failed("the tool refused".into()));
    let chat = Scripted::new(vec![
        calls("echo", json!({})),
        calls("echo", json!({})),
        calls("echo", json!({})),
        says("Could not.\nRESULT: not achieved"),
    ]);
    let echo = Echo::new(vec![failed(), failed(), failed()]);
    let recorder = Recorder::default();
    let run = agent(&chat, &[&echo], None, &recorder).run("Do it");
    assert_eq!(run.status, Status::NotAchieved);
    assert!(results(&run).iter().all(|(_, failed)| *failed));
    assert_eq!(*chat.may_call.lock().unwrap(), [true, true, true, false]);
    assert_eq!(chat.last_user.lock().unwrap()[3], ANSWER_NOW);
    let failures = recorder.events();
    let failures: Vec<&String> = failures
        .iter()
        .filter(|event| event.starts_with("failed"))
        .collect();
    assert_eq!(failures, ["failed echo: the tool refused"; 3]);
}

#[test]
fn bad_arguments_and_unknown_tools_are_answered_without_calling_anything() {
    let chat = Scripted::new(vec![
        calls("echo", json!("not an object")),
        calls("nothing", json!({})),
        says("RESULT: not achieved"),
    ]);
    let echo = Echo::new(vec![]);
    let recorder = Recorder::default();
    let run = agent(&chat, &[&echo], None, &recorder).run("Do it");
    let results = results(&run);
    assert!(results[0].0.contains("not a JSON object") && results[0].1);
    assert_eq!(results[1], ("there is no tool nothing".to_string(), true));
    let events = recorder.events();
    assert!(events[2].starts_with("failed echo: the arguments are not a JSON object"));
    assert_eq!(events[5], "failed nothing: there is no tool nothing");
}

#[test]
fn a_fatal_call_ends_the_run_and_is_answered_for_a_follow_up() {
    let chat = Scripted::new(vec![calls("echo", json!({}))]);
    let echo = Echo::new(vec![Err(ToolError::Fatal("the service is down".into()))]);
    let recorder = Recorder::default();
    let run = agent(&chat, &[&echo], None, &recorder).run("Do it");
    assert_eq!(run.status, Status::Error("the service is down".into()));
    assert_eq!(results(&run), [("the service is down".to_string(), true)]);
    assert_eq!(
        recorder.events(),
        ["turn 0", "call echo", "fatal echo: the service is down"]
    );
}

#[test]
fn turns_run_out_with_one_last_request_for_an_answer() {
    let chat = Scripted::new((0..5).map(|_| calls("echo", json!({}))).collect());
    let echo = Echo::new((0..5).map(|_| ok("again")).collect());
    let run = agent(&chat, &[&echo], None, &Silent).run("Loop forever");
    assert_eq!(run.status, Status::Exhausted);
    assert_eq!(
        *chat.may_call.lock().unwrap(),
        [true, true, true, true, false]
    );
}

#[test]
fn with_a_schema_the_run_ends_in_matching_data() {
    let schema = json!({"type": "object", "required": ["price"], "properties": {"price": {"type": "number"}}});
    let chat = Scripted::new(vec![
        says("The price is 93."),
        calls(
            schema::TOOL,
            json!({"data": {"price": "93"}, "summary": "", "achieved": true}),
        ),
        calls(
            schema::TOOL,
            json!({"data": {"price": 93}, "summary": "93 EUR", "achieved": true}),
        ),
    ]);
    let recorder = Recorder::default();
    let run = agent(&chat, &[], Some(&schema), &recorder).run("Find the price");
    assert_eq!(
        chat.last_user.lock().unwrap()[1],
        RESULT_NOW,
        "text alone is not the end"
    );
    assert!(results(&run)[0].0.contains("data.price must be number"));
    assert_eq!(results(&run)[1], (RETURNED.to_string(), false));
    assert_eq!(run.data, Some(Ok(json!({"price": 93}))));
    assert_eq!(
        (run.status, run.answer.as_str()),
        (Status::Achieved, "93 EUR")
    );
    let events = recorder.events();
    assert!(events[3].starts_with("failed result: The data does not match its schema"));
    assert_eq!(events[6], format!("ok result: {RETURNED}"));
}

#[test]
fn a_result_that_keeps_breaking_its_schema_ends_the_run_without_data() {
    let schema = json!({"type": "object", "required": ["price"], "properties": {"price": {"type": "number"}}});
    let wrong = || {
        calls(
            schema::TOOL,
            json!({"data": {}, "summary": "", "achieved": true}),
        )
    };
    let chat = Scripted::new(vec![wrong(), wrong(), wrong()]);
    let recorder = Recorder::default();
    let run = agent(&chat, &[], Some(&schema), &recorder).run("Find the price");
    assert_eq!(run.status, Status::NotAchieved);
    assert_eq!(
        run.data,
        Some(Err(
            "the result did not match the schema: data is missing \"price\"".into()
        ))
    );
    let failed = recorder.events();
    let failed = failed.iter().filter(|event| event.starts_with("failed"));
    assert_eq!(failed.count(), 3, "every call gets its result event");
}

#[test]
fn the_context_opens_a_new_conversation_only_when_given() {
    let cases: [(&str, &[&str]); 2] = [
        (
            "Working directory: /work\n",
            &[
                "<context>\nWorking directory: /work\n</context>",
                "Goal: First",
                "Goal: Second",
            ],
        ),
        ("", &["Goal: First", "Goal: Second"]),
    ];
    for (context, expected) in cases {
        let chat = Scripted::new(vec![
            says("A.\nRESULT: achieved"),
            says("B.\nRESULT: achieved"),
        ]);
        let agent = Agent {
            context,
            ..agent(&chat, &[], None, &Silent)
        };
        let first = agent.run("First");
        let second = agent.follow_up(first.conversation, "Second");
        assert_eq!(users(&second), expected, "context {context:?}");
    }
}

#[test]
fn with_a_schema_text_twice_ends_without_data() {
    let schema = json!({"type": "object"});
    let chat = Scripted::new(vec![
        says("The price is 93.\nRESULT: achieved"),
        says("It is 93, really.\nRESULT: achieved"),
    ]);
    let run = agent(&chat, &[], Some(&schema), &Silent).run("Find the price");
    assert_eq!(chat.last_user.lock().unwrap()[1], RESULT_NOW);
    assert_eq!(run.status, Status::NotAchieved);
    assert_eq!(run.data, Some(Err(NO_DATA.into())));
    assert_eq!(run.answer, "It is 93, really.");
}

#[test]
fn a_note_goes_at_the_end_with_open_calls_closed_and_the_next_goal_follows() {
    let chat = Scripted::new(vec![
        says("A.\nRESULT: achieved"),
        says("B.\nRESULT: achieved"),
    ]);
    let agent = agent(&chat, &[], None, &Silent);
    let mut first = agent.run("First").conversation;
    // A run that ended mid-turn leaves its call open.
    first.push(calls("echo", json!({})).message());
    let told = agent.tell(Some(first), "<note>changed</note>");
    let messages = told.messages();
    assert!(matches!(
        &messages[messages.len() - 2],
        Message::ToolResult { failed: true, .. }
    ));
    assert_eq!(
        messages.last(),
        Some(&Message::User("<note>changed</note>".into()))
    );
    let second = agent.follow_up(told, "Second");
    assert_eq!(
        users(&second),
        ["Goal: First", "<note>changed</note>", "Goal: Second"]
    );
}

#[test]
fn a_note_before_any_goal_opens_with_the_context() {
    let chat = Scripted::new(vec![says("A.\nRESULT: achieved")]);
    let agent = Agent {
        context: "Working directory: /work",
        ..agent(&chat, &[], None, &Silent)
    };
    let told = agent.tell(None, "<note>changed</note>");
    let run = agent.follow_up(told, "First");
    assert_eq!(
        users(&run),
        [
            "<context>\nWorking directory: /work\n</context>",
            "<note>changed</note>",
            "Goal: First"
        ]
    );
    assert!(matches!(run.conversation.messages()[0], Message::System(_)));
}

fn turn_of(names: &[&str]) -> Completion {
    Completion {
        calls: names
            .iter()
            .enumerate()
            .map(|(n, name)| contracts::chat::ToolCall {
                id: format!("c{n}"),
                name: (*name).into(),
                arguments: json!({}),
            })
            .collect(),
        ..Completion::default()
    }
}

#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn a_picture_a_call_took_follows_every_result_of_its_turn() {
    let chat = Scripted::new(vec![turn_of(&["echo", "echo"]), says("Seen.")]);
    let picture = Image {
        media_type: "image/png".into(),
        data: "AA==".into(),
        width: 1,
        height: 1,
    };
    let echo = Echo::new(vec![
        Ok(contracts::tool::ToolOutput {
            report: "shot".into(),
            state: None,
            image: Some(picture),
        }),
        ok("read"),
    ]);
    let run = agent(&chat, &[&echo], None, &Silent).run("Look");
    let kinds: Vec<&str> = run
        .conversation
        .messages()
        .iter()
        .map(|message| match message {
            Message::ToolResult { .. } => "tool",
            Message::Image { .. } => "image",
            Message::Assistant { .. } => "assistant",
            _ => "other",
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "other",
            "other",
            "assistant",
            "tool",
            "tool",
            "image",
            "assistant"
        ],
        "no message comes between the results of one turn"
    );
}

#[test]
fn a_turn_whose_calls_all_fail_counts_once() {
    let failed = || Err(ToolError::Failed("no such file".into()));
    let chat = Scripted::new(vec![
        turn_of(&["echo", "echo", "echo"]),
        calls("echo", json!({})),
        says("Found it.\nRESULT: achieved"),
    ]);
    let echo = Echo::new(vec![failed(), failed(), failed(), ok("found")]);
    let run = agent(&chat, &[&echo], None, &Silent).run("Find the manifest");
    assert_eq!(*chat.may_call.lock().unwrap(), [true, true, true]);
    assert_eq!(run.status, Status::Achieved);
}

#[test]
fn what_the_model_says_beside_its_calls_is_shown() {
    let chat = Scripted::new(vec![
        Completion {
            text: Some("I will check the lockfile first.".into()),
            ..calls("echo", json!({}))
        },
        says("Done.\nRESULT: achieved"),
    ]);
    let echo = Echo::new(vec![ok("opened")]);
    let recorder = Recorder::default();
    agent(&chat, &[&echo], None, &recorder).run("Fix the build");
    assert_eq!(
        recorder.events(),
        [
            "turn 0",
            "said I will check the lockfile first.",
            "call echo",
            "ok echo: opened",
            "turn 0"
        ],
        "the final answer is the run's, not a said event"
    );
}

/// A strategy that writes down the ceiling and ratio each request is prepared with.
#[derive(Default)]
struct Measured(std::sync::Mutex<Vec<(u64, f64)>>);

impl Strategy for Measured {
    fn prepare(&self, _: &mut Conversation, ceiling: u64, ratio: f64) -> crate::history::Report {
        self.0.lock().unwrap().push((ceiling, ratio));
        vec![]
    }
}

/// A tool whose description is long, as real ones are.
struct Wordy;

impl Tool for Wordy {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "echo".into(),
            description: "d".repeat(40_000),
            parameters: json!({"type": "object"}),
        }
    }
    fn call(&self, _: &Value) -> Result<contracts::tool::ToolOutput, ToolError> {
        ok("opened")
    }
}

#[test]
fn the_tool_specs_count_in_the_ratio_and_against_the_ceiling() {
    let strategy = Measured::default();
    let chat = Scripted::new(vec![
        Completion {
            usage: Usage {
                // The specs (about 10k tokens) and the rules and goal (about 1.2k), as the model
                // counts them.
                input_tokens: 11_300,
                ..Usage::default()
            },
            ..calls("echo", json!({}))
        },
        says("Done.\nRESULT: achieved"),
    ]);
    let run = Agent {
        window: 128_000,
        strategy: &strategy,
        ..agent(&chat, &[&Wordy], None, &Silent)
    }
    .run("Open it");
    assert_eq!(run.status, Status::Achieved);
    let seen = strategy.0.into_inner().unwrap();
    let (first, second) = (seen[0], seen[1]);
    assert!(
        (0.9..1.1).contains(&second.1),
        "the specs are not taken for conversation: ratio {}",
        second.1
    );
    assert!(
        first.0 < 128_000 - 16_384 - 9_000,
        "the specs take their share of the window: ceiling {}",
        first.0
    );
}
