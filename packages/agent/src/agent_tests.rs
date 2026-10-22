use contracts::chat::{Completion, Message, Usage};
use contracts::tool::{Tool, ToolError};
use serde_json::{Value, json};

use super::*;
use crate::fakes::{Echo, Recorder, Scripted, calls, ok, says};
use crate::observer::Silent;

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
    }
}

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
        says("Price is 93 EUR.\nRESULT: achieved"),
    ]);
    let echo = Echo::new(vec![ok("opened")]);
    let recorder = Recorder::default();
    let run = agent(&chat, &[&echo], None, &recorder).run("Find the price");
    assert_eq!(run.status, Status::Achieved);
    assert_eq!(run.answer, "Price is 93 EUR.");
    assert_eq!(results(&run), [("opened\n\nstate".to_string(), false)]);
    assert_eq!(run.spend.calls, 2);
    assert_eq!(
        recorder.events(),
        ["turn 0", "call echo", "ok echo: opened", "turn 0"]
    );
}

#[test]
fn each_turn_is_reported_with_what_it_took() {
    let chat = Scripted::new(vec![Completion {
        text: Some("Done.\nRESULT: achieved".into()),
        usage: Usage {
            output_tokens: 7,
            ..Usage::default()
        },
        ..Completion::default()
    }]);
    let recorder = Recorder::default();
    let run = agent(&chat, &[], None, &recorder).run("Say done");
    assert_eq!(run.spend.output_tokens, 7);
    assert_eq!(recorder.events(), ["turn 7"]);
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
    assert_eq!(*chat.may_call.borrow(), [true, true, true, false]);
    assert_eq!(chat.last_user.borrow()[3], ANSWER_NOW);
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
    assert_eq!(*chat.may_call.borrow(), [true, true, true, true, false]);
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
        chat.last_user.borrow()[1],
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
fn a_new_conversation_opens_with_the_context_and_a_follow_up_does_not_repeat_it() {
    let chat = Scripted::new(vec![
        says("A.\nRESULT: achieved"),
        says("B.\nRESULT: achieved"),
    ]);
    let agent = Agent {
        context: "Working directory: /work\n",
        ..agent(&chat, &[], None, &Silent)
    };
    let first = agent.run("First");
    let second = agent.follow_up(first.conversation, "Second");
    assert_eq!(
        users(&second),
        [
            "<context>\nWorking directory: /work\n</context>\n\nGoal: First",
            "Goal: Second"
        ]
    );
}

#[test]
fn without_context_the_goal_comes_alone() {
    let chat = Scripted::new(vec![
        says("A.\nRESULT: achieved"),
        says("B.\nRESULT: achieved"),
    ]);
    let first = agent(&chat, &[], None, &Silent).run("First");
    let second = agent(&chat, &[], None, &Silent).follow_up(first.conversation, "Second");
    assert_eq!(users(&second), ["Goal: First", "Goal: Second"]);
}

#[test]
fn with_a_schema_text_twice_ends_without_data() {
    let schema = json!({"type": "object"});
    let chat = Scripted::new(vec![
        says("The price is 93.\nRESULT: achieved"),
        says("It is 93, really.\nRESULT: achieved"),
    ]);
    let run = agent(&chat, &[], Some(&schema), &Silent).run("Find the price");
    assert_eq!(chat.last_user.borrow()[1], RESULT_NOW);
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
            "<context>\nWorking directory: /work\n</context>\n\n<note>changed</note>",
            "Goal: First"
        ]
    );
    assert!(matches!(run.conversation.messages()[0], Message::System(_)));
}
