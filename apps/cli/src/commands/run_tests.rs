use agent::agent::Status;
use clap::Parser;
use contracts::chat::{Completion, ToolCall};
use serde_json::{Value, json};
use session::Sessions;

use super::*;
use crate::brake::Halt;
use crate::cli::Cli;
use crate::compose::{Models, assemble};
use crate::exit;
use crate::interrupt::Interrupt;
use crate::mode::Mode;
use crate::output::Output;
use crate::testing::{Captured, Scratch, ScriptedChat};

#[test]
fn an_answer_ends_with_the_code_its_result_line_says() {
    let ended = |status: Status| Ended::of(&status, None, None, 60);
    assert_eq!(ended(Status::Achieved).code, exit::OK);
    assert_eq!(ended(Status::Unstated).code, exit::OK);
    assert_eq!(ended(Status::NotAchieved).code, exit::FAILED);
    assert_eq!(ended(Status::NotAchieved).status, "not_achieved");
    let exhausted = ended(Status::Exhausted);
    assert_eq!(exhausted.code, exit::EXHAUSTED);
    assert!(exhausted.error.unwrap().contains("--max-turns"));
    let error = ended(Status::Error("no network".into()));
    assert_eq!(
        error,
        Ended {
            status: "error",
            code: exit::ENVIRONMENT,
            error: Some("no network".into())
        }
    );
}

#[test]
fn data_that_does_not_match_fails_the_run() {
    let mismatch = Err("the result did not match the schema".to_string());
    let ended = Ended::of(&Status::Achieved, Some(&mismatch), None, 60);
    assert_eq!(ended.code, exit::FAILED);
    let data = Ok(json!({"n": 1}));
    assert_eq!(
        Ended::of(&Status::Achieved, Some(&data), None, 60).code,
        exit::OK
    );
}

#[test]
fn a_halt_decides_the_status_and_exit_code() {
    // Whatever the model said, the halt decides.
    let model_unreachable = Status::Error("the time limit ran out".into());
    let stopped = Status::Error("the person stopped the task".into());
    for (status, halt, ended, code, error) in [
        (
            &model_unreachable,
            Halt::TimedOut,
            "exhausted",
            exit::EXHAUSTED,
            Some("90 s"),
        ),
        (
            &model_unreachable,
            Halt::Closed,
            "interrupted",
            exit::OK,
            None,
        ),
        (
            &stopped,
            Halt::Interrupted,
            "interrupted",
            exit::INTERRUPTED,
            None,
        ),
    ] {
        let got = Ended::of(status, None, Some(halt), 90);
        assert_eq!((got.status, got.code), (ended, code), "{halt:?}");
        match error {
            Some(part) => assert!(got.error.unwrap().contains(part), "{halt:?}"),
            None => assert_eq!(got.error, None, "{halt:?}"),
        }
    }
}

/// What one composed task did: its events, the `-o` file, the session saved.
struct Done {
    code: i32,
    events: Vec<Value>,
    last: String,
    session: Session,
    made: bool,
}

/// Run one task through the whole composition, with a scripted model: one command, then an
/// answer. Nothing but the models is faked.
/// `unwritable`: `-o` names a file in a directory that does not exist.
fn compose_one(mode: Mode, unwritable: bool, schema: Option<Value>) -> Done {
    let scratch = Scratch::new();
    let work = scratch.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let last = match unwritable {
        true => scratch.path().join("missing/last.txt"),
        false => scratch.path().join("last.txt"),
    };
    let args = ["llx", "--json", "-o", last.to_str().unwrap(), "make a file"];
    let options = Cli::try_parse_from(args).unwrap().run;
    let history = Sessions {
        dir: scratch.path().join("sessions"),
    };
    let session = history.start();
    let setup = Setup {
        options,
        choice: choice::Choice::new(crate::providers::Provider::openrouter(), "test/model"),
        workspace: Workspace::new(&work).unwrap(),
        context: String::new(),
        fresh: String::new(),
        schema,
        mode,
        modes: Modes::built_in(),
        rules: Rules::default(),
        // No terminal: nothing here may wait on a person.
        terminal: false,
        asks: false,
        history,
        session,
        ui: None,
    };
    let call = ToolCall {
        id: "c1".into(),
        name: "bash".into(),
        arguments: json!({"command": "echo hi > made.txt"}),
    };
    let chat = ScriptedChat::new(vec![
        Completion {
            calls: vec![call],
            ..Completion::default()
        },
        Completion {
            text: Some("Done.\nRESULT: achieved".into()),
            ..Completion::default()
        },
    ]);
    let models = Models {
        chat: &chat,
        live: None,
    };
    let out = Captured::default();
    let output = Output::with(
        true,
        false,
        Box::new(out.clone()),
        Box::new(Captured::default()),
    );
    let interrupt = Interrupt::new();
    let mut saved = None;
    let (shared, seat) = setup.split();
    let code = assemble(&shared, seat, models, &output, &interrupt, |rig| {
        let code = rig.task("make a file");
        saved = Some(rig.history.load(Some(&rig.session.id)).unwrap());
        code
    });
    Done {
        code,
        events: out
            .text()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect(),
        last: std::fs::read_to_string(&last).unwrap_or_default(),
        session: saved.unwrap(),
        made: work.join("made.txt").exists(),
    }
}

#[test]
fn a_composed_task_reports_in_order_writes_its_answer_and_is_saved() {
    let done = compose_one(crate::modes::built("bypass"), false, None);
    let types: Vec<&str> = done
        .events
        .iter()
        .map(|event| event["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        types,
        [
            "run.started",
            "turn",
            "tool.call",
            "tool.result",
            "turn",
            "run.completed"
        ]
    );
    let completed = &done.events[5];
    assert_eq!(completed["status"], "achieved");
    assert_eq!(completed["exit_code"], done.code);
    assert_eq!(done.code, exit::OK);
    assert!(done.made, "the command ran");
    assert_eq!(done.last, "Done.\n");
    assert_eq!(done.session.turns.len(), 1);
    assert_eq!(done.session.turns[0].status, "achieved");
}

#[test]
fn with_no_one_to_approve_nothing_runs_and_the_model_is_told_why() {
    let done = compose_one(crate::modes::built("default"), false, None);
    let result = &done.events[3];
    assert_eq!(result["type"], "tool.result");
    assert_eq!(result["failed"], true);
    assert!(
        result["output"]
            .as_str()
            .unwrap()
            .contains("no one is at a terminal")
    );
    assert!(!done.made);
}

#[test]
fn a_failed_answer_file_is_in_the_exit_code_the_report_gives() {
    let done = compose_one(crate::modes::built("bypass"), true, None);
    let completed = done.events.last().unwrap();
    assert_eq!(done.code, exit::ENVIRONMENT);
    assert_eq!(completed["exit_code"], exit::ENVIRONMENT);
    assert!(completed["error"].as_str().unwrap().starts_with("-o "));
}

#[test]
fn with_a_schema_and_no_data_the_answer_file_holds_no_prose() {
    let done = compose_one(
        crate::modes::built("bypass"),
        false,
        Some(json!({"type": "object"})),
    );
    assert_eq!(
        done.last, "",
        "prose where data is expected would break the reader"
    );
    assert_ne!(done.code, exit::OK);
}

#[test]
fn a_session_needs_a_terminal_and_no_one_task_option() {
    let options = |args: &[&str]| {
        Cli::try_parse_from(std::iter::once("llx").chain(args.iter().copied()))
            .unwrap()
            .run
    };
    assert!(session_allowed(Some("fix it"), &options(&[]), true).is_ok());
    assert!(session_allowed(None, &options(&[]), true).is_ok());
    let usage = |result: Result<(), Failure>| match result {
        Err(Failure::Usage(why)) => why,
        other => panic!("not a usage error: {other:?}"),
    };
    assert!(usage(session_allowed(Some("x"), &options(&[]), false)).contains("llx exec"));
    assert!(usage(session_allowed(Some("-"), &options(&[]), true)).contains("llx exec"));
    for flag in [&["--json"][..], &["-o", "last.txt"], &["--schema", "{}"]] {
        assert!(usage(session_allowed(None, &options(flag), true)).contains("exec"));
    }
}
