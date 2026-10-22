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
