use contracts::chat::{Completion, Message, Usage};
use contracts::tool::{Input, Tool, ToolError};
use serde_json::{Value, json};

use super::*;
use crate::fakes::Silent;
use crate::fakes::{Echo, Recorder, Scripted, calls, data, done, ok, says, users};

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

#[test]
fn a_reply_without_a_call_is_the_answer_and_ends_the_run() {
    let chat = Scripted::new(vec![
        calls("echo", json!({})),
        Completion {
            usage: Usage {
                output_tokens: 7,
                ..Usage::default()
            },
            ..done("Price is 93 EUR.")
        },
    ]);
    let echo = Echo::new(vec![ok("opened")]);
    let recorder = Recorder::default();
    let run = agent(&chat, &[&echo], None, &recorder).run("Find the price");
    assert_eq!(run.status, Status::Done);
    assert_eq!(run.answer, "Price is 93 EUR.");
    assert_eq!(results(&run), [("opened\n\nstate".to_string(), false)]);
    assert_eq!(run.data, None, "no schema, no data step");
    assert_eq!(run.spend.calls, 2);
    assert_eq!(
        run.spend.output_tokens, 7,
        "the turns' output tokens add up"
    );
    assert_eq!(
        recorder.events(),
        ["turn 0", "call echo", "ok echo: opened", "turn 7",],
        "each turn is reported with what it took"
    );
}

#[test]
fn failed_calls_go_back_to_the_model_which_keeps_its_tools_however_many_fail() {
    // A patch that does not apply, a command refused: the model reads why and tries another way.
    // Nothing takes its tools away; a run is bounded by its turns and its time.
    let failed = || Err(ToolError::Failed("the tool refused".into()));
    let mut script: Vec<Completion> = (0..5).map(|_| calls("echo", json!({}))).collect();
    script.push(calls("echo", json!({})));
    script.push(done("Done."));
    let chat = Scripted::new(script);
    let echo = Echo::new(vec![
        failed(),
        failed(),
        failed(),
        failed(),
        failed(),
        ok("worked"),
    ]);
    let recorder = Recorder::default();
    let run = Agent {
        max_turns: 20,
        ..agent(&chat, &[&echo], None, &recorder)
    }
    .run("Do it");
    assert_eq!(run.status, Status::Done);
    assert_eq!(*chat.calling.lock().unwrap(), ["any"; 7]);
    assert_eq!(users(run.conversation.messages()), ["Goal: Do it"]);
    let failures = recorder.events();
    let failures: Vec<&String> = failures
        .iter()
        .filter(|event| event.starts_with("failed"))
        .collect();
    assert_eq!(failures, ["failed echo: the tool refused"; 5]);
}

#[test]
fn nothing_is_said_to_the_model_but_the_goal_and_its_calls_results() {
    // No reminder, no nudge, no tool that ends the task: the model's first message with no call
    // is the answer, whatever it says.
    let chat = Scripted::new(vec![
        calls("echo", json!({})),
        says("130 of 132 pass; the split is left."),
        says("never asked for"),
    ]);
    let echo = Echo::new(vec![ok("ran")]);
    let recorder = Recorder::default();
    let run = agent(&chat, &[&echo], None, &recorder).run("Split app.ts");
    assert_eq!(
        (run.status, run.answer.as_str()),
        (Status::Done, "130 of 132 pass; the split is left.")
    );
    assert_eq!(users(run.conversation.messages()), ["Goal: Split app.ts"]);
    assert_eq!(chat.replies.lock().unwrap().len(), 1, "no third request");
}

#[test]
fn an_empty_reply_is_not_kept_in_the_conversation() {
    // A refusal or a stray stop: nothing said, nothing called. Sent back, some hosts reject an
    // assistant message with no content.
    let chat = Scripted::new(vec![Completion::default(), done("Done.")]);
    let run = agent(&chat, &[], None, &Silent).run("Do it");
    assert_eq!(run.status, Status::Done);
    let empty = run.conversation.messages().iter().any(|message| {
        matches!(message, Message::Assistant { text, calls, .. } if text.is_none() && calls.is_empty())
    });
    assert!(!empty);
}

#[test]
fn bad_arguments_and_unknown_tools_are_answered_without_calling_anything() {
    let chat = Scripted::new(vec![
        calls("echo", json!("not an object")),
        calls("nothing", json!({})),
        says("Could not."),
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
fn turns_run_out_without_a_last_request_for_an_answer() {
    let chat = Scripted::new((0..5).map(|_| calls("echo", json!({}))).collect());
    let echo = Echo::new((0..5).map(|_| ok("again")).collect());
    let run = agent(&chat, &[&echo], None, &Silent).run("Loop forever");
    assert_eq!(run.status, Status::Exhausted);
    assert_eq!(*chat.calling.lock().unwrap(), ["any"; 5]);
    assert_eq!(chat.last_user.lock().unwrap()[4], "Goal: Loop forever");
}

#[test]
fn with_a_schema_the_data_is_asked_for_after_the_answer_with_the_call_forced() {
    let schema = json!({"type": "object", "required": ["price"], "properties": {"price": {"type": "number"}}});
    let chat = Scripted::new(vec![
        says("The price is 93 EUR."),
        data(&json!({"price": "93"})),
        data(&json!({"price": 93})),
    ]);
    let recorder = Recorder::default();
    let run = agent(&chat, &[], Some(&schema), &recorder).run("Find the price");
    // The work's requests offer no `result`; the data's requests force it.
    assert_eq!(
        *chat.calling.lock().unwrap(),
        ["any", schema::TOOL, schema::TOOL]
    );
    assert_eq!(chat.last_user.lock().unwrap()[1], DATA_NOW);
    assert!(results(&run)[0].0.contains("data.price must be number"));
    assert_eq!(results(&run)[1], (RETURNED.to_string(), false));
    assert_eq!(run.data, Some(Ok(json!({"price": 93}))));
    assert_eq!(
        (run.status, run.answer.as_str()),
        (Status::Done, "The price is 93 EUR.")
    );
    let events = recorder.events();
    assert!(
        events[3].contains("data.price must be number"),
        "{events:?}"
    );
    assert_eq!(events[6], format!("ok result: {RETURNED}"));
}

#[test]
fn a_result_that_keeps_breaking_its_schema_leaves_the_run_without_data() {
    let schema = json!({"type": "object", "required": ["price"], "properties": {"price": {"type": "number"}}});
    let wrong = || data(&json!({}));
    let chat = Scripted::new(vec![says("93."), wrong(), wrong(), wrong(), wrong()]);
    let recorder = Recorder::default();
    let run = agent(&chat, &[], Some(&schema), &recorder).run("Find the price");
    assert_eq!((run.status, run.answer.as_str()), (Status::Done, "93."));
    assert_eq!(
        run.data,
        Some(Err(
            "the result did not match the schema: data is missing \"price\"".into()
        ))
    );
    assert_eq!(
        chat.replies.lock().unwrap().len(),
        1,
        "two fixes, then no more"
    );
    let failed = recorder.events();
    let failed = failed.iter().filter(|event| event.starts_with("failed"));
    assert_eq!(failed.count(), 3, "every call gets its result event");
}

#[test]
fn with_a_schema_a_reply_that_is_not_the_call_leaves_the_run_without_data() {
    let schema = json!({"type": "object"});
    let chat = Scripted::new(vec![says("93."), says("Here it is.")]);
    let run = agent(&chat, &[], Some(&schema), &Silent).run("Find the price");
    assert_eq!(
        run.data,
        Some(Err(
            "the result did not match the schema: no `result` call came".into()
        ))
    );
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
        let chat = Scripted::new(vec![done("A."), done("B.")]);
        let agent = Agent {
            context,
            ..agent(&chat, &[], None, &Silent)
        };
        let first = agent.run("First");
        let second = agent.follow_up(first.conversation, "Second");
        assert_eq!(
            users(second.conversation.messages()),
            expected,
            "context {context:?}"
        );
    }
}

#[test]
fn with_a_schema_turns_that_run_out_leave_no_data_and_no_data_step() {
    let schema = json!({"type": "object"});
    let chat = Scripted::new((0..5).map(|_| calls("echo", json!({}))).collect());
    let echo = Echo::new((0..5).map(|_| ok("again")).collect());
    let run = agent(&chat, &[&echo], Some(&schema), &Silent).run("Find the price");
    assert_eq!(run.status, Status::Exhausted);
    assert_eq!(run.data, None);
    assert_eq!(chat.calling.lock().unwrap().len(), 5);
}

#[test]
fn a_note_goes_at_the_end_with_open_calls_closed_and_the_next_goal_follows() {
    let chat = Scripted::new(vec![done("A."), done("B.")]);
    let agent = agent(&chat, &[], None, &Silent);
    let mut first = agent.run("First").conversation;
    // A run that ended mid-turn leaves its call open.
    let mut open = calls("echo", json!({}));
    open.calls[0].id = "open".into();
    first.push(open.message());
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
        users(second.conversation.messages()),
        ["Goal: First", "<note>changed</note>", "Goal: Second"]
    );
}

#[test]
fn a_note_before_any_goal_opens_with_the_context() {
    let chat = Scripted::new(vec![done("A.")]);
    let agent = Agent {
        context: "Working directory: /work",
        ..agent(&chat, &[], None, &Silent)
    };
    let told = agent.tell(None, "<note>changed</note>");
    let run = agent.follow_up(told, "First");
    assert_eq!(
        users(run.conversation.messages()),
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
fn a_turn_whose_calls_all_fail_counts_once() {
    let failed = || Err(ToolError::Failed("no such file".into()));
    let chat = Scripted::new(vec![
        turn_of(&["echo", "echo", "echo"]),
        calls("echo", json!({})),
        done("Found it."),
    ]);
    let echo = Echo::new(vec![failed(), failed(), failed(), ok("found")]);
    let run = agent(&chat, &[&echo], None, &Silent).run("Find the manifest");
    assert_eq!(*chat.calling.lock().unwrap(), ["any", "any", "any"]);
    assert_eq!(run.status, Status::Done);
}

#[test]
fn what_the_model_says_beside_its_calls_is_shown() {
    let chat = Scripted::new(vec![
        Completion {
            text: Some("I will check the lockfile first.".into()),
            ..calls("echo", json!({}))
        },
        done("Done."),
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
            "turn 0",
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
            input: Input::Json(json!({"type": "object"})),
        }
    }
    fn call(&self, _: &Value) -> Result<ToolOutput, ToolError> {
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
        done("Done."),
    ]);
    let run = Agent {
        window: 128_000,
        strategy: &strategy,
        ..agent(&chat, &[&Wordy], None, &Silent)
    }
    .run("Open it");
    assert_eq!(run.status, Status::Done);
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
