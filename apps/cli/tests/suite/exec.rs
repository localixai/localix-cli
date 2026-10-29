//! `llx exec` as a script runs it, thinking with the scripted model: the answer alone on stdout,
//! the trace on stderr, `--json` lines, and the exit codes of the public contract.

use std::io::Write;
use std::process::{Output, Stdio};

use serde_json::{Value, json};

use crate::common::home::{Home, text};
use crate::common::model::{Model, Reply, said};

/// `exec args…` with `stdin` piped in, run to its end as CI runs it: with no terminal at all (a
/// session of its own, so not even `/dev/tty` is there to ask on).
pub(crate) fn exec(home: &Home, model: &Model, args: &[&str], stdin: &str) -> Output {
    exec_as(home, home.with_model(model, args), stdin)
}

/// `command` run as [`exec`] runs it.
pub(crate) fn run(home: &Home, command: std::process::Command, stdin: &str) -> Output {
    exec_as(home, command, stdin)
}

/// `command` run as [`exec`] runs it.
fn exec_as(home: &Home, mut command: std::process::Command, stdin: &str) -> Output {
    {
        let lead = || {
            // SAFETY: setsid takes nothing and only makes the child a session's leader.
            unsafe { libc::setsid() };
            Ok(())
        };
        // SAFETY: between fork and exec the closure only calls setsid, which is async-signal-safe
        // and touches only the child.
        unsafe { std::os::unix::process::CommandExt::pre_exec(&mut command, lead) };
    }
    let mut child = command
        .current_dir(home.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// A task that runs one command, then answers.
fn one_command() -> Model {
    Model::start(vec![
        Reply::call("bash", json!({"command": "echo hi"})),
        Reply::text("All good.\nRESULT: achieved"),
    ])
}

#[test]
fn the_answer_alone_goes_to_stdout_and_piped_stdin_reaches_the_model() {
    let (home, model) = (Home::new(), one_command());
    let out = exec(&home, &model, &["exec", "-y", "check it"], "some context\n");
    let stderr = text(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    assert_eq!(text(&out.stdout), "All good.\n");
    assert!(stderr.contains("echo hi"), "{stderr}");
    let asked = said(&model, 0);
    assert!(
        asked.contains("<stdin>") && asked.contains("some context"),
        "{asked}"
    );
}

#[test]
fn json_lines_parse_and_end_with_the_exit_code_of_the_process() {
    let (home, model) = (Home::new(), one_command());
    let out = exec(&home, &model, &["exec", "--json", "-y", "check it"], "");
    let events: Vec<Value> = text(&out.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events[0]["type"], "run.started", "{events:?}");
    let last = events.last().unwrap();
    assert_eq!(last["type"], "run.completed", "{events:?}");
    assert_eq!(last["exit_code"], json!(out.status.code().unwrap()));
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn with_no_terminal_a_command_runs_only_with_yes_or_in_bypass_mode() {
    let home = Home::new();
    let made = home.path().join("made.txt");
    for (flags, runs) in [
        (&[][..], false),
        (&["-y"][..], true),
        (&["--mode", "bypass"][..], true),
    ] {
        let _ = std::fs::remove_file(&made);
        let model = Model::start(vec![
            Reply::call("bash", json!({"command": "echo hi > made.txt"})),
            Reply::text("Done.\nRESULT: achieved"),
        ]);
        let args = [&["exec"], flags, &["make it"]].concat();
        let out = exec(&home, &model, &args, "");
        assert_eq!(
            out.status.code(),
            Some(0),
            "{flags:?}: {}",
            text(&out.stderr)
        );
        assert_eq!(made.exists(), runs, "{flags:?}");
        // Refused, the model reads why and what the person can do.
        let told = said(&model, 1);
        assert_eq!(!runs, told.contains("--yes (-y)"), "{flags:?}: {told}");
    }
}

#[test]
fn o_writes_the_answer_and_schema_prints_the_data_alone() {
    let home = Home::new();
    let out = exec(
        &home,
        &one_command(),
        &["exec", "-y", "-o", "answer.txt", "x"],
        "",
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let written = std::fs::read_to_string(home.path().join("answer.txt")).unwrap();
    assert_eq!(written, "All good.\n");
    let schema =
        r#"{"type":"object","properties":{"version":{"type":"string"}},"required":["version"]}"#;
    let model = Model::start(vec![Reply::call(
        "result",
        json!({"data": {"version": "1.2"}, "summary": "Found it.", "achieved": true}),
    )]);
    let args = [
        "exec",
        "--schema",
        schema,
        "-o",
        "data.json",
        "which version",
    ];
    let out = exec(&home, &model, &args, "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "{\"version\":\"1.2\"}\n");
    let written = std::fs::read_to_string(home.path().join("data.json")).unwrap();
    assert_eq!(written, "{\"version\":\"1.2\"}\n");
}

#[test]
fn not_achieved_exits_one_and_out_of_turns_exits_two() {
    let home = Home::new();
    // The first such answer goes back once: the second stands.
    let model = Model::start(vec![
        Reply::text("Could not.\nRESULT: not achieved"),
        Reply::text("Still could not.\nRESULT: not achieved"),
    ]);
    let out = exec(&home, &model, &["exec", "x"], "");
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert_eq!(model.requests().len(), 2);
    let model = Model::start(vec![Reply::call("bash", json!({"command": "echo hi"}))]);
    let out = exec(&home, &model, &["exec", "-y", "--max-turns", "1", "x"], "");
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
}

#[test]
fn a_task_has_no_turn_limit_but_the_one_given() {
    let home = Home::new();
    let mut script: Vec<Reply> = (0..60)
        .map(|_| Reply::call("bash", json!({"command": "true"})))
        .collect();
    script.push(Reply::text("Done.\nRESULT: achieved"));
    let model = Model::start(script);
    let out = exec(&home, &model, &["exec", "-y", "x"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(model.requests().len(), 61);
}

#[test]
fn a_refused_key_exits_three_and_says_what_to_do() {
    let (home, model) = (Home::new(), Model::start(vec![Reply::Refused(401)]));
    let out = exec(&home, &model, &["exec", "x"], "");
    let stderr = text(&out.stderr);
    assert_eq!(out.status.code(), Some(3), "{stderr}");
    assert!(
        stderr.contains("refused the key") && stderr.contains("/login"),
        "{stderr}"
    );
}

#[test]
fn a_picture_that_cannot_be_attached_stops_before_any_model() {
    let home = Home::new();
    std::fs::write(home.path().join("notes.txt"), "not a picture").unwrap();
    for picture in ["missing.png", "notes.txt"] {
        let model = one_command();
        let out = exec(&home, &model, &["exec", "-i", picture, "x"], "");
        let stderr = text(&out.stderr);
        assert_eq!(out.status.code(), Some(3), "{picture}: {stderr}");
        assert!(
            stderr.contains("--image") && stderr.contains(picture),
            "{stderr}"
        );
        assert!(
            model.requests().is_empty(),
            "{picture}: the model was called"
        );
    }
}

#[test]
fn the_time_limit_holds_while_the_task_waits_on_a_quiet_monitor() {
    let home = Home::new();
    let model = Model::start(vec![
        Reply::call(
            "jobs",
            json!({"action": "monitor", "name": "quiet", "command": "sleep 30"}),
        ),
        Reply::text("Watching it.\nRESULT: achieved"),
    ]);
    let started = std::time::Instant::now();
    let out = exec(&home, &model, &["exec", "-y", "--timeout", "2", "x"], "");
    let stderr = text(&out.stderr);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(15),
        "it waited {:?}: {stderr}",
        started.elapsed()
    );
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("time limit"), "{stderr}");
}

#[test]
fn an_agent_is_offered_the_tools_of_the_main_session() {
    let names = |request: &Value| -> Vec<String> {
        (request["tools"].as_array().unwrap().iter())
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect()
    };
    let home = Home::new();
    let model = Model::start(vec![Reply::call(
        "agents",
        json!({"action": "spawn", "task": "say hi", "name": "scout"}),
    )]);
    let out = exec(&home, &model, &["exec", "-y", "start scout"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let requests = model.requests();
    assert!(
        requests.len() >= 3,
        "the agent asked too: {}",
        requests.len()
    );
    let main = names(&requests[0]);
    assert_eq!(main, ["bash", "apply_patch", "agents", "jobs"]);
    for request in &requests {
        assert_eq!(names(request), main, "one list, one prompt cache");
    }
}
