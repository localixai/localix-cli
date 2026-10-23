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

fn shown(name: &str, label: Label, state: State, width: usize) -> String {
    let lines = lines(name, &label, state, width);
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
        State::Done(&outcome),
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
        State::Running,
        80,
    );
    assert_eq!(style::plain(&lines[0]), "• Running cargo test");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].spans[0].style, style::dim());
}

#[test]
fn files_say_what_changed() {
    let edited = done("Edited src/a.rs: replaced 1 occurrence (+3 -1 lines).\n12 fn a() {}");
    let created = done("Created site/index.html (48 lines).");
    insta::assert_snapshot!([
        shown("read", said("Reading", "Read", "src/lib.rs"), State::Done(&done("1 use x;")), 80),
        shown("read", said("Reading", "Read", "src/lib.rs from line 40"), State::Done(&done("40 x")), 80),
        shown("edit", said("Editing", "Edited", "src/a.rs"), State::Done(&edited), 80),
        shown("edit", said("Creating", "Created", "site/index.html"), State::Done(&created), 80),
    ].join("\n"), @r"
    • Read src/lib.rs
    • Read src/lib.rs from line 40
    • Edited src/a.rs
      └ (+3 -1 lines)
    • Created site/index.html
      └ (48 lines)
    ");
}

#[test]
fn edits_show_numbered_coloured_diff_with_context_and_wrapped_code() {
    let outcome = done(
        "Edited a.rs: replaced 1 occurrence (+1 -1 lines).\n\nDiff:\n@@ -181,3 +181,3 @@\n target: 'bun',\n-throw: true,\n+throw: false,\n end",
    );
    let rendered = lines(
        "edit",
        &said("Editing", "Edited", "a.rs"),
        State::Done(&outcome),
        60,
    );
    let screen = rendered
        .iter()
        .map(style::plain)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(screen.contains("181 181   target: 'bun',"), "{screen}");
    assert!(screen.contains("182     - throw: true,"), "{screen}");
    assert!(screen.contains("    182 + throw: false,"), "{screen}");
    assert!(rendered.iter().any(|row| {
        row.spans.iter().any(|span| {
            span.content.contains("throw: true")
                && span.style.bg == Some(ratatui::style::Color::Red)
        })
    }));
    assert!(rendered.iter().any(|row| {
        row.spans.iter().any(|span| {
            span.content.contains("throw: false")
                && span.style.bg == Some(ratatui::style::Color::Green)
        })
    }));
    let long = format!(
        "Created x (1 line).\n\nDiff:\n@@ -0,0 +1,1 @@\n+    {}",
        "x".repeat(80)
    );
    let rendered = lines(
        "edit",
        &said("Editing", "Edited", "x"),
        State::Done(&done(&long)),
        30,
    );
    assert!(
        rendered
            .iter()
            .all(|row| super::super::wrap::line_width(row) <= 30)
    );
    assert_eq!(
        rendered
            .iter()
            .map(style::plain)
            .collect::<String>()
            .matches('x')
            .count(),
        81
    );
}

#[test]
fn diff_backgrounds_fill_changed_rows_including_wraps_but_not_context() {
    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        style::Color,
        widgets::{Paragraph, Widget},
    };

    let report =
        done("Edited x.\n\nDiff:\n@@ -1,2 +1,2 @@\n context\n-old\n+abcdefghijklmnopqrstuvwxyz");
    let rendered = lines(
        "edit",
        &said("Editing", "Edited", "x"),
        State::Done(&report),
        24,
    );
    let area = Rect::new(0, 0, 24, u16::try_from(rendered.len()).unwrap());
    let mut buffer = Buffer::empty(area);
    Paragraph::new(rendered.clone()).render(area, &mut buffer);
    let changed = rendered
        .iter()
        .position(|row| style::plain(row).contains("- old"))
        .unwrap();
    let row = |y: usize| u16::try_from(y).unwrap();
    for x in 0..24 {
        assert_eq!(buffer[(x, row(changed))].bg, Color::Red);
        for y in changed + 1..rendered.len() {
            assert_eq!(buffer[(x, row(y))].bg, Color::Green);
        }
        assert_eq!(buffer[(x, row(changed) - 1)].bg, Color::Reset);
    }
}

#[test]
fn separated_diff_hunks_keep_their_line_numbers_and_a_visible_gap() {
    let report = done(
        "Edited x (+2 -2 lines).\n\nDiff:\n@@ -1,1 +1,1 @@\n-a\n+b\n@@ -300,1 +300,1 @@\n-c\n+d",
    );
    let shown = shown(
        "edit",
        said("Editing", "Edited", "x"),
        State::Done(&report),
        60,
    );
    assert!(shown.contains("    …\n"), "{shown}");
    assert!(shown.contains("300     - c"), "{shown}");
    assert!(shown.contains("    300 + d"), "{shown}");
}

#[test]
fn a_failed_call_shows_why() {
    let outcome = Outcome::Failed {
        why: "old_string was not found in src/a.rs".into(),
        declined: false,
    };
    insta::assert_snapshot!(shown("edit", said("Editing", "Edited", "src/a.rs"), State::Done(&outcome), 80), @r"
    • Edited src/a.rs
      └ old_string was not found in src/a.rs
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
