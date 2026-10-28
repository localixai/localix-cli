use super::*;
use crate::tui::style;

fn done(report: &str) -> Outcome {
    Outcome::Done {
        report: report.into(),
    }
}

/// How a call reads, as the server says it.
fn said(doing: &str, done: &str, subject: &str) -> Label {
    Label {
        doing: doing.into(),
        done: done.into(),
        subject: subject.into(),
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "a test helper takes its arguments as the tests write them, by value"
)]
fn shown(name: &str, label: Label, state: State, width: usize) -> String {
    let lines = lines(name, &label, &state, width);
    lines
        .iter()
        .map(style::plain)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_command_shows_the_ends_of_what_it_printed() {
    let printed: Vec<String> = (1..=12).map(|n| format!("line {n}")).collect();
    let report = format!("exit 0 in 0.4s\n### stdout\n{}", printed.join("\n"));
    insta::assert_snapshot!([
        shown("bash", said("Running", "Ran", "seq 12 | sed 's/^/line /'"), State::Done(&done(&report)), 80),
        shown("bash", said("Running", "Ran", "touch a"), State::Done(&done("exit 0 in 0.0s, no output")), 80),
    ].join("\n"), @r"
    • Ran seq 12 | sed 's/^/line /'
      └ line 1
        line 2
        … +7 lines
        line 10
        line 11
        line 12
    • Ran touch a
      └ (no output)
    ");
}

#[test]
fn a_long_command_wraps_under_a_rule_and_long_output_is_cut() {
    let report = format!("exit 0 in 1.0s\n### stdout\n{}", "x".repeat(100));
    insta::assert_snapshot!(shown("bash", said("Running", "Ran", "find . -maxdepth 3 -not -path '*/node_modules/*' -not -path '*/.git/*' -name '*.rs'"), State::Done(&done(&report)), 48), @r"
    • Ran find . -maxdepth 3 -not -path
      │ '*/node_modules/*' -not -path '*/.git/*'
      │ -name '*.rs'
      └ xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx…
    ");
}

#[test]
fn a_failing_command_ends_with_how_it_exited_in_red() {
    let outcome = done("exit 101 in 2.1s\n### stderr\nerror[E0425]: cannot find value `x`");
    let lines = lines(
        "bash",
        &said("Running", "Ran", "cargo build"),
        &State::Done(&outcome),
        80,
    );
    insta::assert_snapshot!(lines.iter().map(style::plain).collect::<Vec<_>>().join("\n"), @r"
    • Ran cargo build
      └ error[E0425]: cannot find value `x`
        exit 101 in 2.1s
    ");
    assert_eq!(lines[0].spans[0].style, style::failed());
    assert_eq!(lines[2].spans[1].style, style::failed());
}

#[test]
fn a_running_call_has_a_dim_bullet_and_a_present_verb() {
    let lines = lines(
        "bash",
        &said("Running", "Ran", "cargo test"),
        &State::Running,
        80,
    );
    assert_eq!(style::plain(&lines[0]), "• Running cargo test");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].style, style::dim());
}

#[test]
fn a_failed_call_shows_why() {
    let outcome = Outcome::Failed {
        why: "not allowed: the person said no".into(),
        declined: true,
    };
    insta::assert_snapshot!(shown("bash", said("Running", "Ran", "rm -rf dist"), State::Done(&outcome), 80), @r"
    • Ran rm -rf dist
      └ not allowed: the person said no
    ");
}

#[test]
fn window_calls_and_questions() {
    insta::assert_snapshot!([
        shown("window", said("Opening", "Opened", "python3 -q"), State::Running, 80),
        shown("window", said("Working in", "Worked in", "w1: Click Buy; done when the cart shows 1"), State::Done(&done("w1: ### Helper: done (90% sure)\nmore")), 80),
        shown("ask", said("Asking", "Asked", "2 questions"), State::Running, 80),
        shown("ask", said("Asking", "Asked", "1 question"), State::Done(&done("The person answered:\n- colour: Blue")), 80),
    ].join("\n"), @r"
    • Opening python3 -q
    • Worked in w1: Click Buy; done when the cart shows 1
      └ w1: ### Helper: done (90% sure)
    • Asking 2 questions
    • Asked 1 question
      └ - colour: Blue
    ");
}

#[test]
fn control_characters_cannot_reach_the_terminal() {
    let text = shown(
        "bash",
        said("Running", "Ran", "echo \u{1b}[2J"),
        State::Done(&done("exit 0 in 0.0s\n### stdout\n\u{1b}[2Jhi")),
        80,
    );
    assert!(!text.contains('\u{1b}'), "{text}");
}
