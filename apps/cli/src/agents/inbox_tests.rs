use contracts::gate::Open;
use contracts::inbox::Inbox;
use tools::workspace::Workspace;

use super::*;
use crate::jobs::{Jobs, Kit};
use crate::testing::{Captured, Scratch};

fn quiet() -> Output {
    Output::with(
        false,
        false,
        Box::new(Captured::default()),
        Box::new(Captured::default()),
    )
}

/// The main agent's inbox on `output`, with exec's jobs.
fn with_inbox(output: &Output, work: impl FnOnce(&MainInbox, &Registry, &Jobs, &Kit)) {
    let scratch = Scratch::new();
    let workspace = Workspace::new(scratch.path()).unwrap();
    let jobs_output = quiet();
    let kit = Kit {
        workspace: &workspace,
        gate: &Open,
        hidden: &Vec::new,
        output: &jobs_output,
        session: false,
    };
    let (registry, jobs) = (Registry::default(), Jobs::default());
    let inbox = MainInbox {
        registry: &registry,
        jobs: &jobs,
        jobs_kit: &kit,
        output,
        stopped: &|| false,
    };
    work(&inbox, &registry, &jobs, &kit);
}

fn answer(registry: &Registry, text: &str) {
    registry.lock().mail.push_back(("s1".into(), text.into()));
}

#[test]
fn the_person_comes_first_then_the_agents_then_the_jobs() {
    let (ui, events) = crate::tui::test_support::fake_ui();
    let output = quiet().in_ui(Some(ui));
    with_inbox(&output, |inbox, registry, jobs, kit| {
        jobs.post(kit, "<monitor job=\"m\">\ntick\n</monitor>".into());
        answer(
            registry,
            "<agent_result agent=\"a\">\nfound\n</agent_result>",
        );
        std::thread::scope(|scope| {
            scope.spawn(move || {
                if let Ok(Event::TakeSteered { reply, .. }) = events.recv() {
                    let _ = reply.send(vec!["stop, use the other file".into()]);
                }
            });
            let came = inbox.take();
            assert!(came[0].starts_with("stop, use"), "{came:?}");
            assert!(came[1].starts_with("<agent_result"), "{came:?}");
            assert!(came[2].starts_with("<monitor"), "{came:?}");
        });
    });
}

#[test]
fn the_waiting_line_is_said_once_until_the_task_ends() {
    let said = Captured::default();
    let output = Output::with(
        false,
        false,
        Box::new(Captured::default()),
        Box::new(said.clone()),
    );
    with_inbox(&output, |inbox, registry, _, _| {
        answer(registry, "one");
        assert_eq!(inbox.wait(), ["one"]);
        answer(registry, "two");
        assert_eq!(inbox.wait(), ["two"]);
        assert_eq!(said.text().matches("Waiting").count(), 1, "{}", said.text());
        // The task ends however it ends (stopped, out of turns, done): the next one says it again.
        registry.settled();
        answer(registry, "three");
        assert_eq!(inbox.wait(), ["three"]);
        assert_eq!(said.text().matches("Waiting").count(), 2, "{}", said.text());
    });
}
