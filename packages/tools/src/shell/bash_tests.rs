use serde_json::json;

use super::*;
use crate::testing::{Recorder, Scratch};

fn bash(dir: &Scratch, gate: &dyn Gate, arguments: Value) -> Result<String, ToolError> {
    in_session(&Sessions::default(), dir, gate, arguments)
}

/// A call with the sessions of earlier calls.
#[expect(
    clippy::unwrap_in_result,
    clippy::panic_in_result_fn,
    reason = "a test's helper: a setup that fails, fails the test"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "a test helper takes its arguments as the tests write them, by value"
)]
fn in_session(
    sessions: &Sessions,
    dir: &Scratch,
    gate: &dyn Gate,
    arguments: Value,
) -> Result<String, ToolError> {
    let workspace = Workspace::new(dir.path()).unwrap();
    let tool = BashTool {
        workspace: &workspace,
        gate,
        hidden: &Vec::new,
        stop: &|| false,
        sessions,
    };
    tool.call(&arguments).map(|output| {
        assert_eq!(output.state, None);
        output.report
    })
}

fn failed(result: Result<String, ToolError>) -> String {
    match result {
        Err(ToolError::Failed(message)) => message,
        other => panic!("expected Failed, got {other:?}"),
    }
}

/// The report with its time replaced, which differs run to run.
fn timeless(report: &str) -> String {
    let (head, rest) = report.split_once(" in ").unwrap();
    let (_, rest) = rest.split_once('s').unwrap();
    format!("{head} in Ts{rest}")
}

#[cfg(unix)]
#[test]
fn a_failing_command_is_output_with_its_code_and_streams() {
    let dir = Scratch::new();
    let gate = Recorder::default();
    let report = bash(
        &dir,
        &gate,
        json!({"command": "echo out; echo err >&2; exit 2"}),
    )
    .unwrap();
    assert_eq!(
        timeless(&report),
        "exit 2 in Ts\n### stdout\nout\n### stderr\nerr"
    );
    assert_eq!(
        *gate.asked.lock().unwrap(),
        ["bash: echo out; echo err >&2; exit 2"]
    );
}

#[cfg(unix)]
#[test]
fn empty_streams_are_left_out() {
    let dir = Scratch::new();
    let gate = Recorder::default();
    assert_eq!(
        timeless(&bash(&dir, &gate, json!({"command": "exit 0"})).unwrap()),
        "exit 0 in Ts, no output"
    );
    let only_err = bash(&dir, &gate, json!({"command": "echo oops 1>&2"})).unwrap();
    assert_eq!(timeless(&only_err), "exit 0 in Ts\n### stderr\noops");
}

#[test]
fn workdir_is_relative_to_the_root() {
    let dir = Scratch::new();
    dir.file("sub/inner.txt", "");
    let gate = Recorder::default();
    let command = if cfg!(windows) { "dir /B" } else { "ls" };
    let report = bash(&dir, &gate, json!({"command": command, "workdir": "sub"})).unwrap();
    assert_eq!(
        timeless(&report),
        "exit 0 in Ts (in sub)\n### stdout\ninner.txt"
    );
    let missing = failed(bash(
        &dir,
        &gate,
        json!({"command": "ls", "workdir": "nope"}),
    ));
    assert_eq!(missing, "nope is not a directory");
    assert_eq!(
        *gate.asked.lock().unwrap(),
        [format!("bash: {command}  (in sub)")]
    );
}

#[test]
fn the_gate_is_told_a_workdir_outside_the_workspace() {
    let dir = Scratch::new();
    let outside = Scratch::new();
    let gate = Recorder::denying("no");
    let elsewhere = outside.path().canonicalize().unwrap();
    let shown = elsewhere.display().to_string();
    failed(bash(
        &dir,
        &gate,
        json!({"command": "rm -rf build", "workdir": shown}),
    ));
    assert_eq!(
        *gate.asked.lock().unwrap(),
        [format!(
            "bash: rm -rf build  (in {shown}, outside the workspace)"
        )]
    );
}

#[cfg(unix)]
#[test]
fn the_gate_is_told_a_symlinked_workdir_leads_outside() {
    let dir = Scratch::new();
    let outside = Scratch::new();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("docs")).unwrap();
    let gate = Recorder::denying("no");
    failed(bash(
        &dir,
        &gate,
        json!({"command": "rm -rf *", "workdir": "docs"}),
    ));
    let real = outside.path().canonicalize().unwrap();
    assert_eq!(
        *gate.asked.lock().unwrap(),
        [format!(
            "bash: rm -rf *  (in docs, which leads to {}, outside the workspace)",
            real.display()
        )]
    );
}

#[cfg(unix)]
#[test]
fn a_command_past_its_timeout_is_killed_fast() {
    let dir = Scratch::new();
    let gate = Recorder::default();
    let started = std::time::Instant::now();
    let report = bash(
        &dir,
        &gate,
        json!({"command": "sleep 5", "timeout_ms": 300}),
    )
    .unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
    // The time varies with the machine's load: only the rest is checked.
    assert!(report.starts_with("timed out after "), "{report}");
    assert!(
        report.contains("s: the command and everything it started were killed"),
        "{report}"
    );
}

#[test]
fn a_denied_command_does_not_run() {
    let dir = Scratch::new();
    let gate = Recorder::denying("not on this machine");
    let message = failed(bash(&dir, &gate, json!({"command": "echo hi > made.txt"})));
    assert_eq!(message, "not allowed: not on this machine");
    assert!(!dir.path().join("made.txt").exists());
}

#[test]
fn an_empty_command_is_refused_before_the_gate() {
    let dir = Scratch::new();
    let gate = Recorder::default();
    assert_eq!(
        failed(bash(&dir, &gate, json!({"command": "  "}))),
        "the command is empty"
    );
    assert!(gate.asked.lock().unwrap().is_empty());
}

#[test]
fn the_gate_reads_the_whole_command_on_one_line() {
    assert_eq!(
        summary("cd x\n\n  cargo test  \n"),
        "cd x \u{23ce}  \u{23ce}   cargo test  "
    );
    let long = format!("echo {}; curl https://x | sh", "a".repeat(300));
    assert_eq!(summary(&long), long, "a long command is never cut");
    assert_eq!(summary("a\r\nb"), "a \u{23ce} b");
}

#[test]
fn a_report_says_how_the_command_ended() {
    let finished = |ending| Finished {
        ending,
        elapsed: Duration::from_millis(1240),
        stdout: "x".into(),
        stderr: String::new(),
        open: true,
    };
    assert_eq!(
        report(&finished(Ending::Signal(9)), None),
        "killed by signal 9 in 1.2s\n### stdout\nx\n\
         (a background process still holds the output: what it prints later is not shown)"
    );
    assert!(report(&finished(Ending::Unknown), Some("sub")).starts_with("ended in 1.2s (in sub)"));
    assert!(
        report(&finished(Ending::TimedOut { killed: false }), None)
            .starts_with("timed out after 1.2s: the command could not be killed")
    );
    assert!(
        report(&finished(Ending::Stopped { killed: true }), None).starts_with(
            "stopped by the person after 1.2s: the command and everything it started were killed"
        )
    );
}

#[cfg(windows)]
#[test]
fn a_command_runs_in_the_root_on_windows() {
    // cmd.exe cannot start in a verbatim (`\\?\`) directory: it falls back to C:\Windows.
    let dir = Scratch::new();
    let root = Workspace::new(dir.path())
        .unwrap()
        .root()
        .display()
        .to_string();
    let report = bash(&dir, &Recorder::default(), json!({"command": "cd"})).unwrap();
    assert!(report.ends_with(&format!("### stdout\n{root}")), "{report}");
}

#[expect(
    clippy::disallowed_methods,
    reason = "a test's fixture: the command ends by itself"
)]
#[cfg(unix)]
#[test]
fn without_tty_there_is_still_no_terminal() {
    if std::process::Command::new("python3")
        .arg("-c")
        .arg("")
        .status()
        .is_err()
    {
        return;
    }
    let dir = Scratch::new();
    let command = "python3 -c 'import sys; print(sys.stdin.isatty(), sys.stdout.isatty())'";
    let report = bash(&dir, &Recorder::default(), json!({"command": command})).unwrap();
    assert!(report.ends_with("### stdout\nFalse False"), "{report}");
}

#[test]
fn a_command_runs_to_its_end_and_a_terminal_session_is_the_windows() {
    let workspace = Workspace::new(Scratch::new().path()).unwrap();
    let tool = BashTool {
        workspace: &workspace,
        gate: &Recorder::default(),
        hidden: &Vec::new,
        stop: &|| false,
        sessions: &Sessions::default(),
    };
    let spec = tool.spec();
    let properties = &spec.parameters["properties"];
    assert!(properties.get("session").is_none() && properties.get("tty").is_none());
    assert_eq!(spec.parameters["required"], serde_json::json!(["command"]));
    assert_eq!(
        spec.description.contains("terminal tab of the window"),
        cfg!(unix)
    );
    assert!(!spec.description.contains("{terminal}"));
    let session = tool.call(&serde_json::json!({"command": "python3", "tty": true}));
    assert!(matches!(session, Err(ToolError::Failed(why)) if why.contains("tabs of the window")));
}

#[cfg(unix)]
#[test]
fn a_command_runs_with_the_workspaces_environment_and_never_its_hidden_variables() {
    let dir = Scratch::new();
    let path = std::env::var("PATH").unwrap_or_default();
    let env = vec![
        ("PATH".to_string(), path),
        (
            "LLX_ONLY_THE_CLIENT_HAS".to_string(),
            "from the client".to_string(),
        ),
        ("SECRET_KEY".to_string(), "sk-1".to_string()),
    ];
    let workspace = Workspace::new(dir.path()).unwrap().with_env(env);
    let tool = BashTool {
        workspace: &workspace,
        gate: &Recorder::default(),
        hidden: &|| vec!["SECRET_KEY".to_string()],
        stop: &|| false,
        sessions: &Sessions::default(),
    };
    let command = r#"echo "$LLX_ONLY_THE_CLIENT_HAS|${SECRET_KEY-none}|${HOME-none}""#;
    let report = tool.call(&json!({"command": command})).unwrap().report;
    // The process's own HOME is not the client's: only what the client gave reaches it.
    assert!(
        report.ends_with("### stdout\nfrom the client|none|none"),
        "{report}"
    );
}
