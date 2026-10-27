use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use contracts::chat::{Completion, Message, ToolCall};
use contracts::inbox::Inbox;
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use crate::agent::Agent;
use crate::fakes::Silent;
use crate::fakes::{Recorder, Scripted, says};

/// Where the calls of a test's tools run, whichever tool they belong to: how many run now,
/// the most that ever ran at once, and the order they started and ended in.
#[derive(Default)]
struct Room {
    running: Mutex<(usize, usize)>,
    changed: Condvar,
    log: Mutex<Vec<String>>,
}

impl Room {
    fn peak(&self) -> usize {
        self.running.lock().unwrap().1
    }

    fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

/// A tool whose calls go through a room. With `meet` above one, a call waits (a few seconds
/// at most, and no longer than it takes) for that many calls to have run at once, so a test of
/// concurrency does not race the scheduler. Otherwise a call holds a moment, so that an overlap
/// with another would show in the room's peak.
struct Meet<'a> {
    room: &'a Room,
    parallel: bool,
    meet: usize,
}

fn look(room: &Room) -> Meet<'_> {
    Meet {
        room,
        parallel: true,
        meet: 1,
    }
}

impl Tool for Meet<'_> {
    fn spec(&self) -> ToolSpec {
        let name = if self.parallel { "look" } else { "change" };
        ToolSpec {
            name: name.into(),
            description: String::new(),
            parameters: json!({"type": "object"}),
        }
    }

    #[expect(
        clippy::unwrap_in_result,
        clippy::disallowed_methods,
        reason = "a test's fake tool: a poisoned lock fails the test, and a call alone works 20 ms so that calls could overlap"
    )]
    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let id = arguments["id"].as_str().unwrap_or("").to_string();
        let room = self.room;
        let mut counts = room.running.lock().unwrap();
        counts.0 += 1;
        counts.1 = counts.1.max(counts.0);
        room.log.lock().unwrap().push(format!("start {id}"));
        room.changed.notify_all();
        if self.meet > 1 {
            counts = room
                .changed
                .wait_timeout_while(counts, Duration::from_secs(5), |(_, peak)| {
                    *peak < self.meet
                })
                .unwrap()
                .0;
        } else {
            drop(counts);
            std::thread::sleep(Duration::from_millis(20));
            counts = room.running.lock().unwrap();
        }
        let together = counts.1 >= 2;
        counts.0 -= 1;
        room.log.lock().unwrap().push(format!("end {id}"));
        drop(counts);
        Ok(ToolOutput {
            report: format!("{id} {}", if together { "together" } else { "alone" }),
            ..ToolOutput::default()
        })
    }

    fn parallel(&self) -> bool {
        self.parallel
    }
}

fn call(id: &str, name: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: name.into(),
        arguments: json!({"id": id}),
    }
}

fn turn(calls: Vec<ToolCall>) -> Completion {
    Completion {
        calls,
        ..Completion::default()
    }
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn results(messages: &[Message]) -> Vec<(String, String)> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::ToolResult { id, content, .. } => Some((id.clone(), content.clone())),
            _ => None,
        })
        .collect()
}

fn agent<'a>(
    chat: &'a Scripted,
    tools: &'a [&'a dyn Tool],
    inbox: Option<&'a dyn Inbox>,
) -> Agent<'a> {
    Agent {
        chat,
        tools,
        context: "",
        max_turns: 6,
        observer: &Silent,
        schema: None,
        inbox,
        window: 0,
        strategy: &crate::fakes::Keep,
    }
}

#[test]
fn calls_that_may_run_together_do_and_answer_in_the_models_order() {
    let room = Room::default();
    let look = Meet {
        meet: 2,
        ..look(&room)
    };
    let chat = Scripted::new(vec![
        turn(vec![call("a", "look"), call("b", "look")]),
        says("Done.\nRESULT: achieved"),
    ]);
    let tools: [&dyn Tool; 1] = [&look];
    let run = agent(&chat, &tools, None).run("look twice");
    assert_eq!(room.peak(), 2, "both ran at once");
    let results = results(run.conversation.messages());
    assert_eq!(
        results,
        [
            ("a".to_string(), "a together".to_string()),
            ("b".to_string(), "b together".to_string())
        ]
    );
}

#[test]
fn a_call_that_may_not_runs_alone_between_the_others() {
    let room = Room::default();
    let change = Meet {
        parallel: false,
        ..look(&room)
    };
    let look = look(&room);
    let chat = Scripted::new(vec![
        turn(vec![
            call("a", "look"),
            call("x", "change"),
            call("b", "look"),
        ]),
        says("Done.\nRESULT: achieved"),
    ]);
    let tools: [&dyn Tool; 2] = [&look, &change];
    let run = agent(&chat, &tools, None).run("look, change, look");
    assert_eq!(room.peak(), 1, "no two calls overlapped");
    assert_eq!(
        room.log(),
        ["start a", "end a", "start x", "end x", "start b", "end b"]
    );
    let results: Vec<(String, String)> = results(run.conversation.messages());
    assert_eq!(
        results,
        [
            ("a".to_string(), "a alone".to_string()),
            ("x".to_string(), "x alone".to_string()),
            ("b".to_string(), "b alone".to_string())
        ]
    );
}

#[test]
fn a_batch_is_watched_call_by_call() {
    let room = Room::default();
    let look = look(&room);
    let chat = Scripted::new(vec![
        turn(vec![call("a", "look"), call("b", "look")]),
        says("Done."),
    ]);
    let recorder = Recorder::default();
    let tools: [&dyn Tool; 1] = [&look];
    let agent = Agent {
        observer: &recorder,
        ..agent(&chat, &tools, None)
    };
    agent.run("look");
    let events: Vec<String> = recorder
        .events()
        .into_iter()
        .filter(|event| !event.starts_with("turn"))
        .collect();
    assert_eq!(events[..2], ["call look", "call look"]);
    assert_eq!(
        events.iter().filter(|e| e.starts_with("ok look")).count(),
        2
    );
}

/// An inbox that gives out what the test put in, and waits once for what comes later.
#[derive(Default)]
struct Mail {
    now: Mutex<Vec<String>>,
    later: Mutex<Vec<String>>,
    waiting: AtomicBool,
    waits: AtomicUsize,
}

impl Inbox for Mail {
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.now.lock().unwrap())
    }

    fn expecting(&self) -> bool {
        self.waiting.load(Ordering::SeqCst)
    }

    fn wait(&self) -> Vec<String> {
        self.waits.fetch_add(1, Ordering::SeqCst);
        self.waiting.store(false, Ordering::SeqCst);
        std::mem::take(&mut *self.later.lock().unwrap())
    }
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn users(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::User(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn what_comes_in_is_said_before_the_next_turn() {
    let room = Room::default();
    let look = look(&room);
    let mail = Mail::default();
    mail.now
        .lock()
        .unwrap()
        .push("also check the README".into());
    let chat = Scripted::new(vec![turn(vec![call("a", "look")]), says("Done.")]);
    let tools: [&dyn Tool; 1] = [&look];
    let run = agent(&chat, &tools, Some(&mail)).run("look");
    assert_eq!(
        users(run.conversation.messages()),
        ["Goal: look", "also check the README"]
    );
}

#[test]
fn an_answer_while_work_is_still_to_come_waits_for_it_and_goes_on() {
    let mail = Mail::default();
    mail.waiting.store(true, Ordering::SeqCst);
    mail.later
        .lock()
        .unwrap()
        .push("<agent_result>found it</agent_result>".into());
    let chat = Scripted::new(vec![
        says("Waiting for the agent."),
        says("It found it.\nRESULT: achieved"),
    ]);
    let run = agent(&chat, &[], Some(&mail)).run("delegate");
    assert_eq!(mail.waits.load(Ordering::SeqCst), 1);
    assert_eq!(run.answer, "It found it.");
    assert_eq!(
        users(run.conversation.messages()),
        ["Goal: delegate", "<agent_result>found it</agent_result>"]
    );
}

/// A chat model that answers from a script, and puts a message in the inbox while it answers
/// the first time: the person writes while the model writes.
struct Interrupted<'a> {
    script: Scripted,
    mail: &'a Mail,
}

impl contracts::chat::Chat for Interrupted<'_> {
    #[expect(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn complete(
        &self,
        request: contracts::chat::Request,
    ) -> Result<Completion, contracts::chat::ChatError> {
        let reply = self.script.complete(request);
        if self.script.may_call.lock().unwrap().len() == 1 {
            self.mail
                .now
                .lock()
                .unwrap()
                .push("run the tests too".into());
        }
        reply
    }
}

#[test]
fn a_message_that_comes_with_the_answer_is_not_lost() {
    let mail = Mail::default();
    let chat = Interrupted {
        script: Scripted::new(vec![says("Done."), says("Done, with the tests too.")]),
        mail: &mail,
    };
    let agent = Agent {
        chat: &chat,
        tools: &[],
        context: "",
        max_turns: 6,
        observer: &Silent,
        schema: None,
        inbox: Some(&mail),
        window: 0,
        strategy: &crate::fakes::Keep,
    };
    let run = agent.run("work");
    assert_eq!(run.answer, "Done, with the tests too.");
    assert_eq!(
        users(run.conversation.messages()),
        ["Goal: work", "run the tests too"]
    );
}

#[test]
fn an_answer_on_the_last_turn_is_kept_and_nothing_is_waited_for() {
    let mail = Mail::default();
    mail.waiting.store(true, Ordering::SeqCst);
    let chat = Scripted::new(vec![says("Here is what I have.\nRESULT: not achieved")]);
    let run = Agent {
        max_turns: 1,
        ..agent(&chat, &[], Some(&mail))
    }
    .run("delegate");
    assert_eq!(mail.waits.load(Ordering::SeqCst), 0, "no wait");
    assert_eq!(run.answer, "Here is what I have.");
    assert_eq!(run.status, crate::agent::Status::NotAchieved);
}
