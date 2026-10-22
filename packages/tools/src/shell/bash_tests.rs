use serde_json::json;

use super::*;
use crate::testing::{Recorder, Scratch};

fn bash(dir: &Scratch, gate: &dyn Gate, arguments: Value) -> Result<String, ToolError> {
    let workspace = Workspace::new(dir.path()).unwrap();
    let tool = BashTool {
        workspace: &workspace,
        gate,
        hidden: &[],
        stop: &|| false,
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
        *gate.asked.borrow(),
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
    assert_eq!(*gate.asked.borrow(), [format!("bash: {command}  (in sub)")]);
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
        *gate.asked.borrow(),
        [format!(
            "bash: rm -rf build  (in {shown}, outside the workspace)"
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

#[cfg(unix)]
#[test]
fn long_output_is_cut_in_the_middle() {
    let dir = Scratch::new();
    let gate = Recorder::default();
    let command = "i=0; while [ $i -lt 10000 ]; do echo line $i; i=$((i+1)); done";
    let report = bash(&dir, &gate, json!({"command": command})).unwrap();
    assert!(report.len() < 11_000, "{} bytes", report.len());
    assert!(report.contains("### stdout\nline 0\nline 1\n"));
    assert!(report.contains(" characters cut ...\n"));
    assert!(report.ends_with("line 9999"));
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
    assert!(gate.asked.borrow().is_empty());
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

#[test]
fn the_description_names_the_shell_of_this_system() {
    let workspace = Workspace::new(Scratch::new().path()).unwrap();
    let tool = BashTool {
        workspace: &workspace,
        gate: &Recorder::default(),
        hidden: &[],
        stop: &|| false,
    };
    assert!(tool.spec().description.contains(&format!("(`{SHELL}`)")));
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
