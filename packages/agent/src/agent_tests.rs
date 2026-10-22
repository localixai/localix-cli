use std::cell::RefCell;

use contracts::chat::{ChatError, Completion, Message, Request, ToolCall};
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use super::*;

/// A chat model that replies from a script, and remembers what each request allowed.
struct Scripted {
    replies: RefCell<Vec<Completion>>,
    may_call: RefCell<Vec<bool>>,
    last_user: RefCell<Vec<String>>,
}

impl Scripted {
    fn new(replies: Vec<Completion>) -> Self {
        Self {
            replies: RefCell::new(replies.into_iter().rev().collect()),
            may_call: RefCell::new(vec![]),
            last_user: RefCell::new(vec![]),
        }
    }
}

impl Chat for Scripted {
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        self.may_call.borrow_mut().push(request.may_call);
        let user = request
            .conversation
            .iter()
            .rev()
            .find_map(|message| match message {
                Message::User(text) => Some(text.clone()),
                _ => None,
            });
        self.last_user.borrow_mut().push(user.unwrap_or_default());
        self.replies
            .borrow_mut()
            .pop()
            .ok_or_else(|| ChatError::Unavailable("the script ran out".into()))
    }
}

fn says(text: &str) -> Completion {
    Completion {
        text: Some(text.into()),
        ..Completion::default()
    }
}

fn calls(name: &str, arguments: Value) -> Completion {
    Completion {
        calls: vec![ToolCall {
            id: "c1".into(),
            name: name.into(),
            arguments,
        }],
        ..Completion::default()
    }
}

/// A tool that answers each call from a script.
struct Echo(RefCell<Vec<Result<ToolOutput, ToolError>>>);

impl Echo {
    fn new(outcomes: Vec<Result<ToolOutput, ToolError>>) -> Self {
        Self(RefCell::new(outcomes.into_iter().rev().collect()))
    }
}

impl Tool for Echo {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "echo".into(),
            description: "Says what it did.".into(),
            parameters: json!({"type": "object"}),
        }
    }
    fn call(&self, _: &Value) -> Result<ToolOutput, ToolError> {
        self.0.borrow_mut().pop().expect("an unexpected call")
    }
}

fn ok(report: &str) -> Result<ToolOutput, ToolError> {
    Ok(ToolOutput {
        report: report.into(),
        state: Some("state".into()),
    })
}

fn agent<'a>(
    chat: &'a Scripted,
    tools: &'a [&'a dyn Tool],
    schema: Option<&'a Value>,
) -> Agent<'a> {
    Agent {
        chat,
        tools,
        max_turns: 5,
        on_call: &|_| {},
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

#[test]
fn a_reply_without_a_call_is_the_answer_and_its_result_line_the_status() {
    let chat = Scripted::new(vec![
        calls("echo", json!({})),
        says("Price is 93 €.\nRESULT: achieved"),
    ]);
    let echo = Echo::new(vec![ok("opened")]);
    let run = agent(&chat, &[&echo], None).run("Find the price");
    assert_eq!(run.status, Status::Achieved);
    assert_eq!(run.answer, "Price is 93 €.");
    assert_eq!(results(&run), [("opened\n\nstate".to_string(), false)]);
    assert_eq!(run.spend.calls, 2);
}

#[test]
fn failed_calls_go_back_to_the_model_and_too_many_ask_for_an_answer() {
    let failed = || Err(ToolError::Failed("the page refused".into()));
    let chat = Scripted::new(vec![
        calls("echo", json!({})),
        calls("echo", json!({})),
        calls("echo", json!({})),
        says("Could not.\nRESULT: not achieved"),
    ]);
    let echo = Echo::new(vec![failed(), failed(), failed()]);
    let run = agent(&chat, &[&echo], None).run("Do it");
    assert_eq!(run.status, Status::NotAchieved);
    assert!(results(&run).iter().all(|(_, failed)| *failed));
    assert_eq!(*chat.may_call.borrow(), [true, true, true, false]);
    assert_eq!(chat.last_user.borrow()[3], ANSWER_NOW);
}

#[test]
fn bad_arguments_and_unknown_tools_are_answered_without_calling_anything() {
    let chat = Scripted::new(vec![
        calls("echo", json!("not an object")),
        calls("nothing", json!({})),
        says("RESULT: not achieved"),
    ]);
    let echo = Echo::new(vec![]);
    let run = agent(&chat, &[&echo], None).run("Do it");
    let results = results(&run);
    assert!(results[0].0.contains("not a JSON object") && results[0].1);
    assert_eq!(results[1], ("there is no tool nothing".to_string(), true));
}

#[test]
fn a_fatal_call_ends_the_run() {
    let chat = Scripted::new(vec![calls("echo", json!({}))]);
    let echo = Echo::new(vec![Err(ToolError::Fatal(
        "the decision model is down".into(),
    ))]);
    let run = agent(&chat, &[&echo], None).run("Do it");
    assert_eq!(
        run.status,
        Status::Error("the decision model is down".into())
    );
}

#[test]
fn turns_run_out_with_one_last_request_for_an_answer() {
    let chat = Scripted::new((0..5).map(|_| calls("echo", json!({}))).collect());
    let echo = Echo::new((0..5).map(|_| ok("again")).collect());
    let run = agent(&chat, &[&echo], None).run("Loop forever");
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
            json!({"data": {"price": 93}, "summary": "93 €", "achieved": true}),
        ),
    ]);
    let run = agent(&chat, &[], Some(&schema)).run("Find the price");
    assert_eq!(
        chat.last_user.borrow()[1],
        RESULT_NOW,
        "text alone is not the end"
    );
    assert!(results(&run)[0].0.contains("data.price must be number"));
    assert_eq!(run.data, Some(Ok(json!({"price": 93}))));
    assert_eq!(
        (run.status, run.answer.as_str()),
        (Status::Achieved, "93 €")
    );
}

#[test]
fn a_follow_up_goes_on_in_the_same_conversation() {
    let chat = Scripted::new(vec![
        says("A.\nRESULT: achieved"),
        says("B.\nRESULT: achieved"),
    ]);
    let first = agent(&chat, &[], None).run("First");
    let second = agent(&chat, &[], None).follow_up(first.conversation, "Second");
    let users: Vec<&Message> = second
        .conversation
        .messages()
        .iter()
        .filter(|message| matches!(message, Message::User(_)))
        .collect();
    assert_eq!(
        users,
        [
            &Message::User("Goal: First".into()),
            &Message::User("Goal: Second".into())
        ]
    );
}
