use contracts::tool::ToolOutput;
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

fn summary<'a>(agent: &'a AgentSpend, decision: &'a Spend) -> Summary<'a> {
    Summary {
        status: "achieved",
        code: 0,
        answer: "All tests pass.",
        data: None,
        error: None,
        agent,
        decision,
        millis: 10,
        session: "1-ab",
    }
}

fn run(output: &Output) {
    let start = Start {
        goal: "run the tests",
        session: "1-ab",
        model: "m",
        decision_model: "d",
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
    let (agent, decision) = (AgentSpend::default(), Spend::default());
    output.completed(&summary(&agent, &decision));
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
    assert!(err.text().contains("▸ bash cargo test"));
    assert!(!out.text().contains("▸"));
}

#[test]
fn a_person_reads_the_trace_on_stderr_and_the_answer_alone_on_stdout() {
    let (out, err) = (Captured::default(), Captured::default());
    let output = Output::with(false, false, Box::new(out.clone()), Box::new(err.clone()));
    run(&output);
    assert_eq!(out.text(), "All tests pass.\n");
    let trace = err.text();
    assert!(trace.contains("s ▸ bash cargo test\n"), "{trace}");
    assert!(trace.contains("achieved in "));
    assert!(
        !trace.contains("exit 0 in 1.0s"),
        "a success shows only with -v"
    );
}

#[test]
fn a_closed_stdout_is_noticed_and_never_panics() {
    let err = Captured::default();
    let output = Output::with(true, false, Box::new(Closed), Box::new(err.clone()));
    assert!(!output.closed());
    run(&output);
    assert!(output.closed());
}
