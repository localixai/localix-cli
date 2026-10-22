use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Line;

use super::*;
use crate::tui::style;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn approval() -> Select {
    Select::new(
        vec![
            Line::from("Run this command?"),
            Line::from("  cargo test --workspace"),
        ],
        vec![
            Choice::new("Yes").key('y'),
            Choice::new("Yes, and don't ask again for bash").key('a'),
            Choice::new("No, and tell llx what to do differently")
                .key('n')
                .types(),
        ],
        "esc to say no",
    )
}

fn shown(select: &Select, width: usize) -> String {
    select
        .render(width)
        .0
        .iter()
        .map(style::plain)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn an_approval_reads_as_a_short_list() {
    insta::assert_snapshot!(shown(&approval(), 60), @r"
    Run this command?
      cargo test --workspace

    ❯ 1. Yes
      2. Yes, and don't ask again for bash
      3. No, and tell llx what to do differently

      ↑↓ to choose · enter to confirm · esc to say no
    ");
}

#[test]
fn arrows_numbers_and_keys_pick() {
    let mut select = approval();
    assert_eq!(select.key(key(KeyCode::Down)), None);
    assert_eq!(select.key(key(KeyCode::Enter)), Some(Picked::Option(1)));
    assert_eq!(
        approval().key(key(KeyCode::Char('1'))),
        Some(Picked::Option(0))
    );
    assert_eq!(
        approval().key(key(KeyCode::Char('y'))),
        Some(Picked::Option(0))
    );
    assert_eq!(approval().key(key(KeyCode::Char('9'))), None);
    assert_eq!(approval().key(key(KeyCode::Esc)), Some(Picked::Cancelled));
}

#[test]
fn an_option_that_types_opens_a_line_and_esc_goes_back() {
    let mut select = approval();
    assert_eq!(select.key(key(KeyCode::Char('n'))), None);
    for c in "use make".chars() {
        select.key(key(KeyCode::Char(c)));
    }
    let (rows, cursor) = select.render(60);
    assert_eq!(style::plain(&rows[cursor.unwrap().0]), "  ❯ use make");
    assert_eq!(cursor.unwrap().1, 12);
    assert_eq!(
        select.key(key(KeyCode::Enter)),
        Some(Picked::Typed(2, "use make".into()))
    );
    let mut select = approval();
    select.key(key(KeyCode::Char('n')));
    assert_eq!(select.key(key(KeyCode::Esc)), None);
    assert_eq!(select.key(key(KeyCode::Esc)), Some(Picked::Cancelled));
}

#[test]
fn a_long_option_wraps_under_its_label() {
    let select = Select::new(
        vec![],
        vec![Choice::new("A choice whose label is long").detail("and its detail too")],
        "esc to skip",
    );
    insta::assert_snapshot!(shown(&select, 30), @r"
    ❯ 1. A choice whose label is
         long and its detail too

      ↑↓ to choose · enter to confirm · esc to skip
    ");
}

#[test]
fn a_line_alone_shows_no_list_and_esc_cancels() {
    let mut select = Select::new(
        vec![Line::from("Paste the key")],
        vec![Choice::new("Type it").types()],
        "esc to cancel",
    )
    .hidden(true)
    .typing(0);
    for c in "sk-1".chars() {
        select.key(key(KeyCode::Char(c)));
    }
    insta::assert_snapshot!(shown(&select, 40), @r"
    Paste the key

      ❯ ••••

      enter to send · esc to cancel
    ");
    assert_eq!(select.key(key(KeyCode::Esc)), Some(Picked::Cancelled));
}
