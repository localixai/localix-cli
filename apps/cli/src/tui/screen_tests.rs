use ratatui::text::Line;

use super::*;

const ROWS: u16 = 8;
const COLUMNS: u16 = 30;

/// A terminal that keeps what scrolled off the top.
fn terminal() -> vt100::Parser {
    vt100::Parser::new(ROWS, COLUMNS, 100)
}

fn lines(rows: &[&str]) -> Vec<Line<'static>> {
    rows.iter().map(|row| Line::from(row.to_string())).collect()
}

/// Every row the terminal holds, scrollback first, without trailing blank rows.
fn everything(parser: &mut vt100::Parser) -> String {
    let mut rows = vec![];
    let back = {
        parser.screen_mut().set_scrollback(usize::MAX);
        parser.screen().scrollback()
    };
    for offset in (1..=back).rev() {
        parser.screen_mut().set_scrollback(offset);
        rows.push(parser.screen().rows(0, COLUMNS).next().unwrap_or_default());
    }
    parser.screen_mut().set_scrollback(0);
    rows.extend(parser.screen().rows(0, COLUMNS));
    while rows.last().is_some_and(|row| row.trim().is_empty()) {
        rows.pop();
    }
    rows.join("\n")
}

fn feed(parser: &mut vt100::Parser, screen: &mut Screen<Vec<u8>>) {
    parser.process(screen.output());
}

#[test]
fn history_goes_above_and_the_live_area_is_redrawn_in_place() {
    let mut screen = Screen::new(Vec::new(), false);
    screen
        .frame(
            &[],
            &lines(&["• Running cargo test", "", "❯ "]),
            Some(Cursor { row: 2, column: 2 }),
            ROWS as usize,
            COLUMNS as usize,
        )
        .unwrap();
    screen
        .frame(
            &lines(&["❯ fix the tests", ""]),
            &lines(&["• Running cargo test", "", "❯ "]),
            Some(Cursor { row: 2, column: 2 }),
            ROWS as usize,
            COLUMNS as usize,
        )
        .unwrap();
    screen
        .frame(
            &lines(&["• Ran cargo test", "  └ ok", ""]),
            &lines(&["", "❯ next"]),
            Some(Cursor { row: 1, column: 6 }),
            ROWS as usize,
            COLUMNS as usize,
        )
        .unwrap();
    let mut parser = terminal();
    feed(&mut parser, &mut screen);
    insta::assert_snapshot!(everything(&mut parser), @r"
    ❯ fix the tests

    • Ran cargo test
      └ ok


    ❯ next
    ");
    assert_eq!(parser.screen().cursor_position(), (6, 6));
    assert!(!parser.screen().hide_cursor());
}

#[test]
fn history_longer_than_the_screen_scrolls_into_the_scrollback() {
    let mut screen = Screen::new(Vec::new(), false);
    let many: Vec<String> = (1..=12).map(|n| format!("row {n}")).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    screen
        .frame(
            &[],
            &lines(&["❯ "]),
            Some(Cursor { row: 0, column: 2 }),
            ROWS as usize,
            COLUMNS as usize,
        )
        .unwrap();
    screen
        .frame(
            &lines(&many),
            &lines(&["• Working", "❯ "]),
            None,
            ROWS as usize,
            COLUMNS as usize,
        )
        .unwrap();
    let mut parser = terminal();
    feed(&mut parser, &mut screen);
    let all = everything(&mut parser);
    assert!(all.starts_with("row 1\nrow 2\n"), "{all}");
    assert!(all.trim_end().ends_with("row 12\n• Working\n❯"), "{all}");
    assert_eq!(
        all.matches("❯").count(),
        1,
        "the old live area is gone: {all}"
    );
    assert!(parser.screen().hide_cursor());
}

#[test]
fn a_live_area_taller_than_the_screen_keeps_its_last_rows() {
    let mut screen = Screen::new(Vec::new(), false);
    let tall: Vec<String> = (1..=20).map(|n| format!("live {n}")).collect();
    let tall: Vec<&str> = tall.iter().map(String::as_str).collect();
    screen
        .frame(
            &[],
            &lines(&tall),
            Some(Cursor { row: 19, column: 0 }),
            ROWS as usize,
            COLUMNS as usize,
        )
        .unwrap();
    screen
        .frame(
            &[],
            &lines(&["❯ "]),
            Some(Cursor { row: 0, column: 2 }),
            ROWS as usize,
            COLUMNS as usize,
        )
        .unwrap();
    let mut parser = terminal();
    feed(&mut parser, &mut screen);
    assert_eq!(parser.screen().contents().trim(), "❯");
}

#[test]
fn clearing_leaves_the_history_and_a_shown_cursor() {
    let mut screen = Screen::new(Vec::new(), false);
    screen
        .frame(
            &lines(&["• done"]),
            &lines(&["", "❯ "]),
            None,
            ROWS as usize,
            COLUMNS as usize,
        )
        .unwrap();
    screen.clear().unwrap();
    let mut parser = terminal();
    feed(&mut parser, &mut screen);
    assert_eq!(everything(&mut parser), "• done");
    assert_eq!(parser.screen().cursor_position(), (1, 0));
    assert!(!parser.screen().hide_cursor());
}

#[test]
fn every_frame_is_one_synchronized_update() {
    let mut screen = Screen::new(Vec::new(), true);
    screen
        .frame(&[], &lines(&["❯ "]), None, ROWS as usize, COLUMNS as usize)
        .unwrap();
    let text = String::from_utf8(screen.output().clone()).unwrap();
    assert!(
        text.starts_with("\x1b[?2026h") && text.ends_with("\x1b[?2026l"),
        "{text:?}"
    );
}

#[test]
fn successful_edit_displays_the_actual_before_and_after_in_the_terminal() {
    use crate::tui::{app::App, event::Event, test_support::Term};
    use contracts::{gate::Open, tool::Tool};
    use serde_json::json;

    let dir = crate::testing::Scratch::new();
    dir.file("config.rs", "target: 'bun',\nthrow: true,\nend\n");
    let workspace = tools::workspace::Workspace::new(dir.path()).unwrap();
    let tool = tools::fs::EditTool {
        workspace: &workspace,
        gate: &Open,
    };
    let args = json!({"path":"config.rs", "old_string":"throw: true", "new_string":"throw: false"});
    let outcome = tool.call(&args);
    let now = std::time::Instant::now();
    let mut app = App::new(80, 30, vec![]);
    let mut term = Term::new(&app);
    app.event(
        Event::Result {
            name: "edit".into(),
            args,
            outcome,
        },
        now,
    );
    let shown = term.draw(&mut app, now);
    assert!(shown.contains("• Edited config.rs"), "{shown}");
    assert!(shown.contains("- throw: true,"), "{shown}");
    assert!(shown.contains("+ throw: false,"), "{shown}");
    assert!(shown.contains("target: 'bun',"), "{shown}");
}

#[test]
fn consecutive_reads_form_a_compact_chain_even_when_they_finish_in_separate_frames() {
    use contracts::tool::ToolOutput;
    use serde_json::json;

    use crate::tui::app::App;
    use crate::tui::event::Event;
    use crate::tui::test_support::Term;

    let now = std::time::Instant::now();
    let mut app = App::new(60, 24, vec![]);
    let mut term = Term::new(&app);
    app.event(Event::TaskStarted("inspect the files".into()), now);
    for (path, offset) in [("src/main.rs", 1), ("src/lib.rs", 20), ("README.md", 1)] {
        let args = json!({"path": path, "offset": offset});
        app.event(
            Event::Call {
                name: "read".into(),
                args: args.clone(),
            },
            now,
        );
        term.draw(&mut app, now);
        app.event(
            Event::Result {
                name: "read".into(),
                args,
                outcome: Ok(ToolOutput {
                    report: "contents".into(),
                    state: None,
                    image: None,
                }),
            },
            now,
        );
        term.draw(&mut app, now);
    }
    app.event(Event::Answer("Found the entry points.".into()), now);
    let shown = term.draw(&mut app, now);
    assert!(shown.contains("• Read src/main.rs from line 1\n  ↳ Read src/lib.rs from line 20\n  ↳ Read README.md from line 1\n\n• Found the entry points."), "{shown}");
}

#[test]
fn grouped_calls_keep_outputs_and_failures_and_approvals_break_the_chain() {
    use contracts::tool::{ToolError, ToolOutput};
    use serde_json::json;

    use crate::tui::app::App;
    use crate::tui::event::Event;
    use crate::tui::test_support::Term;

    let now = std::time::Instant::now();
    let mut app = App::new(60, 40, vec![]);
    let mut term = Term::new(&app);
    app.event(Event::TaskStarted("check".into()), now);
    for (command, outcome) in [
        (
            "one",
            Ok(ToolOutput {
                report: "exit 0 in 0.0s\n### stdout\nfirst output".into(),
                state: None,
                image: None,
            }),
        ),
        (
            "two",
            Ok(ToolOutput {
                report: "exit 0 in 0.0s\n### stdout\nsecond output".into(),
                state: None,
                image: None,
            }),
        ),
        ("bad", Err(ToolError::Failed("permission denied".into()))),
        (
            "three",
            Ok(ToolOutput {
                report: "exit 0 in 0.0s\n### stdout\nthird output".into(),
                state: None,
                image: None,
            }),
        ),
    ] {
        app.event(
            Event::Result {
                name: "bash".into(),
                args: json!({"command": command}),
                outcome,
            },
            now,
        );
        term.draw(&mut app, now);
    }
    let (reply, _answer) = std::sync::mpsc::channel();
    app.event(
        Event::Approve {
            tool: "bash".into(),
            summary: "run four".into(),
            reply,
        },
        now,
    );
    let shown = term.draw(&mut app, now);
    assert!(shown.contains("• Ran one\n  └ first output\n  ↳ Ran two\n  └ second output\n\n• Ran bad\n  └ permission denied\n\n• Ran three"), "{shown}");
    assert!(
        shown.contains("Run this command?") && shown.contains("run four"),
        "{shown}"
    );
    app.event(Event::Dismiss, now);
    app.event(
        Event::Result {
            name: "bash".into(),
            args: json!({"command": "four"}),
            outcome: Ok(ToolOutput {
                report: "exit 0 in 0.0s".into(),
                state: None,
                image: None,
            }),
        },
        now,
    );
    let shown = term.draw(&mut app, now);
    assert!(shown.contains("third output\n\n• Ran four"), "{shown}");
}

#[test]
fn a_row_wider_than_the_terminal_is_redrawn_without_leaving_copies() {
    let mut screen = Screen::new(Vec::new(), false);
    let wide = "x".repeat(70);
    let live = lines(&["• Run this command?", &wide, "❯ 1. Yes", "  2. No"]);
    for _ in 0..3 {
        screen
            .frame(&[], &live, None, ROWS as usize, COLUMNS as usize)
            .unwrap();
    }
    let mut parser = terminal();
    feed(&mut parser, &mut screen);
    let all = everything(&mut parser);
    assert_eq!(all.matches("Run this command?").count(), 1, "{all}");
}
