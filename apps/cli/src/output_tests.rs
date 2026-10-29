use agent::observer::Observer;
use contracts::chat::{ToolCall, Usage};
use contracts::tool::{ToolError, ToolOutput};
use serde_json::json;

use super::*;
use crate::testing::{Captured, Closed};

fn call() -> ToolCall {
    ToolCall {
        id: "c1".into(),
        name: "bash".into(),
        arguments: json!({"command": "cargo test"}),
    }
}

fn summary(agent: &Spend) -> Summary<'_> {
    Summary {
        status: "done",
        code: 0,
        answer: "All tests pass.",
        data: None,
        error: None,
        agent,
        millis: 10,
        session: "1-ab",
    }
}

fn run(output: &Output) {
    let start = Start {
        goal: "run the tests",
        session: "1-ab",
        model: "m",
        workspace: "/w",
    };
    output.started(&start);
    output.call(&call());
    let done = Ok(ToolOutput {
        report: "exit 0 in 1.0s".into(),
        state: None,
    });
    output.result(&call(), &done);
    output.turn(&Usage::default());
    let agent = Spend::default();
    output.completed(&summary(&agent));
}

#[test]
fn json_is_one_event_per_line_on_stdout_and_nothing_on_stderr() {
    let (out, err) = (Captured::default(), Captured::default());
    let output = Output::with(true, false, Box::new(out.clone()), Box::new(err.clone()));
    run(&output);
    let types: Vec<String> = out
        .text()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["type"].to_string())
        .collect();
    assert_eq!(
        types,
        [
            "\"run.started\"",
            "\"tool.call\"",
            "\"tool.result\"",
            "\"turn\"",
            "\"run.completed\""
        ]
    );
    assert_eq!(err.text(), "");
}

#[test]
fn verbose_json_also_traces_on_stderr() {
    let (out, err) = (Captured::default(), Captured::default());
    let output = Output::with(true, true, Box::new(out.clone()), Box::new(err.clone()));
    run(&output);
    assert!(err.text().contains("• Ran cargo test"));
    assert!(!out.text().contains("• Ran"));
}

#[test]
fn a_person_reads_the_trace_on_stderr_and_the_answer_alone_on_stdout() {
    let (out, err) = (Captured::default(), Captured::default());
    let output = Output::with(false, false, Box::new(out.clone()), Box::new(err.clone()));
    run(&output);
    assert_eq!(out.text(), "All tests pass.\n");
    let trace = err.text();
    insta::assert_snapshot!(trace, @r"
    • Ran cargo test
      └ (no output)

    ─ Worked for 0.0s · 0 turns · $0.0000 ──────────────────────────────────────────────────────────────
      session  1-ab
    ");
}

#[test]
fn a_closed_stdout_is_noticed_and_never_panics() {
    let err = Captured::default();
    let output = Output::with(true, false, Box::new(Closed), Box::new(err));
    assert!(!output.closed());
    run(&output);
    assert!(output.closed());
}

#[test]
fn what_the_model_says_beside_its_calls_is_in_the_trace_and_not_in_the_events() {
    let (out, err) = (Captured::default(), Captured::default());
    let output = Output::with(false, false, Box::new(out.clone()), Box::new(err.clone()));
    output.said("I will check the lockfile first.");
    output.call(&call());
    assert!(
        err.text().contains("I will check the lockfile first."),
        "{}",
        err.text()
    );
    assert_eq!(out.text(), "", "stdout gets the answer alone");
    let (out, err) = (Captured::default(), Captured::default());
    let output = Output::with(true, false, Box::new(out.clone()), Box::new(err.clone()));
    output.said("I will check the lockfile first.");
    assert_eq!(
        (out.text(), err.text()),
        (String::new(), String::new()),
        "no event without a schema change"
    );
}

#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn a_turn_tells_the_interface_its_tokens() {
    let (ui, frames) = crate::testing::fake_ui();
    let output = Output::new(false, false).in_ui(Some(ui.for_lane("lane")));
    let usage = Usage {
        input_tokens: 1000,
        output_tokens: 200,
        ..Default::default()
    };
    Observer::turn(&output, &usage);
    let events = crate::testing::events(&frames);
    let spent = events.into_iter().find_map(|(lane, event)| match event {
        Event::Spent(tokens) => Some((lane, tokens)),
        _ => None,
    });
    assert_eq!(spent, Some((Some("lane".to_string()), 1200)));
}

#[test]
fn a_task_keeps_what_it_did_each_call_and_turn_with_its_time() {
    let output = Output::with(
        false,
        false,
        Box::new(Captured::default()),
        Box::new(Captured::default()),
    );
    run(&output);
    // A task that just began: what came before was the last one's.
    output.take_steps();
    let call = call();
    output.call(&call);
    output.result(&call, &Err(ToolError::Failed("no".into())));
    output.turn(&Usage {
        millis: 1200,
        input_tokens: 900,
        output_tokens: 40,
        ..Usage::default()
    });
    let steps = output.take_steps();
    assert_eq!(steps.len(), 2, "{steps:?}");
    assert_eq!((steps[0].what.as_str(), steps[0].failed), ("bash", true));
    assert!(steps[0].detail.starts_with("Running"), "{steps:?}");
    assert_eq!((steps[1].what.as_str(), steps[1].ms), ("model", 1200));
    assert_eq!(steps[1].detail, "900 in · 40 out");
    assert!(output.take_steps().is_empty(), "taken once");
}

#[test]
fn on_a_github_runner_the_trace_cannot_issue_workflow_commands() {
    // A file or a model's answer can hold `::add-mask::` or `##[add-matcher]`: the runner reads
    // such a line on either stream as a command. The trace on stderr
    // is what is guarded here; stdout is pinned below to stay as it is.
    let err = Captured::default();
    let mut output = Output::with(
        false,
        true,
        Box::new(Captured::default()),
        Box::new(err.clone()),
    );
    let text = "::add-mask::abc\n  ::set-env name=X::1\n##[add-matcher]m.json\nok :: fine\n";
    output.runner = Runner::GitHub;
    output.note(text);
    for line in err.text().lines() {
        let line = line.trim_start();
        assert!(
            !line.starts_with("::") && !line.starts_with("##["),
            "{line:?}"
        );
    }
    assert!(err.text().contains("ok :: fine"));
    assert_eq!(
        err.text().replace('\u{200b}', ""),
        text,
        "what the person reads stays"
    );
}

#[test]
fn on_a_github_runner_the_answer_on_stdout_stays_byte_exact() {
    // Stdout is the answer and the data, which a pipe reads as they are: only the trace is guarded.
    let (out, err) = (Captured::default(), Captured::default());
    let mut output = Output::with(false, false, Box::new(out.clone()), Box::new(err));
    output.runner = Runner::GitHub;
    let agent = Spend::default();
    let answer = "::set-output name=x::1\n##[add-matcher]m.json";
    output.completed(&Summary {
        answer,
        ..summary(&agent)
    });
    assert_eq!(out.text(), format!("{answer}\n"));
}
