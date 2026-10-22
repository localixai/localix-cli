use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Span;

use super::*;
use crate::tui::style;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn with(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

fn typed(composer: &mut Composer, text: &str) {
    for c in text.chars() {
        composer.key(key(KeyCode::Char(c)));
    }
}

fn shown(composer: &Composer, width: usize) -> (String, (usize, usize)) {
    let (lines, cursor) = composer.render(Span::raw("❯ "), "Type a task", width);
    let text = lines
        .iter()
        .map(style::plain)
        .collect::<Vec<_>>()
        .join("\n");
    (text, cursor)
}

#[test]
fn enter_sends_and_remembers() {
    let mut composer = Composer::default();
    typed(&mut composer, "fix the tests");
    assert_eq!(
        composer.key(key(KeyCode::Enter)),
        Edit::Sent("fix the tests".into())
    );
    assert!(composer.is_empty());
    composer.key(key(KeyCode::Up));
    assert_eq!(composer.text(), "fix the tests");
    composer.key(key(KeyCode::Down));
    assert_eq!(composer.text(), "");
}

#[test]
fn new_lines_come_from_shift_enter_ctrl_j_a_backslash_and_a_paste() {
    let mut composer = Composer::default();
    typed(&mut composer, "one");
    composer.key(with(KeyCode::Enter, KeyModifiers::SHIFT));
    typed(&mut composer, "two");
    composer.key(with(KeyCode::Char('j'), KeyModifiers::CONTROL));
    typed(&mut composer, "three\\");
    composer.key(key(KeyCode::Enter));
    composer.paste("four\r\nfive");
    assert_eq!(
        composer.key(key(KeyCode::Enter)),
        Edit::Sent("one\ntwo\nthree\nfour\nfive".into())
    );
}

#[test]
fn the_cursor_moves_by_character_word_and_line() {
    let mut composer = Composer::default();
    typed(&mut composer, "run cargo test");
    composer.key(with(KeyCode::Left, KeyModifiers::ALT));
    typed(&mut composer, "the ");
    assert_eq!(composer.text(), "run cargo the test");
    composer.key(with(KeyCode::Char('a'), KeyModifiers::CONTROL));
    composer.key(with(KeyCode::Char('k'), KeyModifiers::CONTROL));
    assert_eq!(composer.text(), "");
    typed(&mut composer, "a b c");
    composer.key(with(KeyCode::Char('w'), KeyModifiers::CONTROL));
    assert_eq!(composer.text(), "a b ");
    composer.key(key(KeyCode::Backspace));
    assert_eq!(composer.text(), "a b");
}

#[test]
fn up_and_down_move_between_lines_before_walking_the_history() {
    let mut composer = Composer::default();
    typed(&mut composer, "earlier");
    composer.key(key(KeyCode::Enter));
    composer.paste("first line\nsecond");
    composer.key(key(KeyCode::Up));
    typed(&mut composer, "!");
    assert_eq!(composer.text(), "first !line\nsecond");
    composer.key(key(KeyCode::Up));
    assert_eq!(composer.text(), "earlier");
    composer.key(key(KeyCode::Down));
    assert_eq!(
        composer.text(),
        "first !line\nsecond",
        "what was typed comes back"
    );
}

#[test]
fn it_renders_with_a_placeholder_then_wraps_with_the_cursor_where_it_types() {
    let mut composer = Composer::default();
    assert_eq!(shown(&composer, 20), ("❯ Type a task".into(), (0, 2)));
    typed(&mut composer, "abcdefghijklmnopqrstuvwxyz");
    insta::assert_snapshot!(shown(&composer, 12).0, @r"
    ❯ abcdefghij
      klmnopqrst
      uvwxyz
    ");
    assert_eq!(shown(&composer, 12).1, (2, 8));
    for _ in 0..16 {
        composer.key(key(KeyCode::Left));
    }
    assert_eq!(
        shown(&composer, 12).1,
        (1, 2),
        "the first character of a row"
    );
    composer.clear();
    typed(&mut composer, "abcdefghij");
    assert_eq!(
        shown(&composer, 12).1,
        (1, 2),
        "a full row puts the cursor on the next"
    );
}

#[test]
fn wide_characters_keep_their_columns() {
    let mut composer = Composer::default();
    typed(&mut composer, "\u{65e5}\u{672c}\u{8a9e}");
    assert_eq!(shown(&composer, 20).1, (0, 8));
}
