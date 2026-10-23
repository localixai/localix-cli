use std::sync::mpsc::{Receiver, channel};
use std::time::Instant;

use crossterm::event::KeyCode;

use super::*;
use crate::tui::app::App;
use crate::tui::event::Event;
use crate::tui::test_support::{Term, key, typed};

/// A long list as `/login` shows it: a name, then the id and a note.
fn providers() -> Vec<(String, String)> {
    let mut named: Vec<(String, String)> = [
        ("OpenAI", "openai"),
        ("Anthropic", "anthropic"),
        ("OpenRouter", "openrouter"),
        ("DeepSeek", "deepseek"),
        ("Hyperopen", "hyperopen"),
    ]
    .iter()
    .map(|(name, id)| (name.to_string(), format!("{id} · not signed in")))
    .collect();
    named.extend((1..=25).map(|n| (format!("Vendor {n:02}"), format!("vendor-{n:02} · signed"))));
    named
}

/// `options` put to the person under `title`; the app, and where the pick goes.
fn choose(title: &str, options: Vec<(String, String)>) -> (App, Receiver<Option<usize>>) {
    let mut app = App::new(80, 40, vec![]);
    let (reply, picked) = channel();
    let event = Event::Choose {
        title: title.into(),
        options,
        marked: None,
        reply,
    };
    app.event(event, Instant::now());
    (app, picked)
}

fn press(app: &mut App, codes: &[KeyCode]) {
    for code in codes {
        app.event(key(*code), Instant::now());
    }
}

fn screen(app: &mut App) -> String {
    Term::new(app).draw(app, Instant::now())
}

#[test]
fn a_long_list_scrolls_to_its_end_and_back() {
    let (mut app, _picked) = choose("Provider", providers());
    for _ in 0..29 {
        press(&mut app, &[KeyCode::Down]);
    }
    let shown = screen(&mut app);
    assert!(shown.contains("❯ Vendor 25"), "{shown}");
    assert!(shown.contains("↑ 18 more"), "{shown}");
    assert_eq!(shown.matches(" more").count(), 1, "nothing below:\n{shown}");
    press(&mut app, &[KeyCode::Home]);
    let shown = screen(&mut app);
    assert!(shown.contains("❯ OpenAI"), "{shown}");
    assert!(shown.contains("↓ 18 more"), "{shown}");
    press(&mut app, &[KeyCode::End]);
    assert!(screen(&mut app).contains("❯ Vendor 25"));
    press(&mut app, &[KeyCode::Home, KeyCode::PageDown]);
    assert!(
        screen(&mut app).contains("❯ Vendor 08"),
        "a page is 12 rows"
    );
    press(&mut app, &[KeyCode::PageUp]);
    assert!(screen(&mut app).contains("❯ OpenAI"));
}

#[test]
fn typing_filters_the_list_by_name_and_id_prefix_first() {
    let (mut app, _picked) = choose("Provider", providers());
    typed(&mut app, "OPEN", Instant::now());
    insta::assert_snapshot!(screen(&mut app), @"
    • Provider

      Search: OPEN▏  3 matches

    ❯ OpenAI openai · not signed in
      OpenRouter openrouter · not signed in
      Hyperopen hyperopen · not signed in

      type to search · ↑↓ to choose · enter to confirm · esc to clear
    ");
}

#[test]
fn backspace_edits_the_query_and_esc_clears_it_then_cancels() {
    let (mut app, picked) = choose("Provider", providers());
    typed(&mut app, "deepx", Instant::now());
    assert!(screen(&mut app).contains("No provider matches 'deepx'"));
    press(&mut app, &[KeyCode::Backspace]);
    let shown = screen(&mut app);
    assert!(shown.contains("Search: deep▏  1 match"), "{shown}");
    assert!(shown.contains("❯ DeepSeek"), "{shown}");
    press(&mut app, &[KeyCode::Esc]);
    let shown = screen(&mut app);
    assert!(
        shown.contains("Search: ▏"),
        "the query is cleared:\n{shown}"
    );
    assert!(shown.contains("OpenAI"), "every row is back:\n{shown}");
    assert!(picked.try_recv().is_err(), "the first Esc does not cancel");
    press(&mut app, &[KeyCode::Esc]);
    assert_eq!(picked.try_recv(), Ok(None));
}

#[test]
fn enter_picks_the_row_marked_among_the_matches() {
    let (mut app, picked) = choose("Provider", providers());
    // Digits are part of the query here, not picks.
    typed(&mut app, "vendor-1", Instant::now());
    press(&mut app, &[KeyCode::Down, KeyCode::Enter]);
    assert_eq!(
        picked.try_recv(),
        Ok(Some(15)),
        "Vendor 11, the second match"
    );
}

#[test]
fn nothing_matching_enter_picks_nothing() {
    let (mut app, picked) = choose("Model (DeepSeek)", providers());
    typed(&mut app, "xyz", Instant::now());
    assert!(screen(&mut app).contains("No model matches 'xyz'"));
    press(&mut app, &[KeyCode::Enter]);
    assert!(picked.try_recv().is_err());
}

#[test]
fn a_short_list_is_numbered_and_takes_no_query() {
    let options = vec![
        ("Keep it".to_string(), String::new()),
        ("Sign in again".to_string(), String::new()),
    ];
    let (mut app, picked) = choose("Signed in", options);
    typed(&mut app, "s", Instant::now());
    let shown = screen(&mut app);
    assert!(!shown.contains("Search"), "{shown}");
    assert!(shown.contains("❯ 1. Keep it"), "{shown}");
    press(&mut app, &[KeyCode::Char('2')]);
    assert_eq!(picked.try_recv(), Ok(Some(1)));
}

#[test]
fn prefix_matches_come_first_then_substrings_then_word_starts() {
    let rows: Vec<[String; 2]> = ["Google AI Studio", "Vegas", "Gasline", "Other"]
        .iter()
        .map(|name| keys(name, ""))
        .collect();
    assert_eq!(ranked("GAS", &rows), [2, 1, 0]);
    assert_eq!(ranked("", &rows), [0, 1, 2, 3]);
}

#[test]
fn a_row_is_found_by_its_label_and_the_lead_of_its_detail_only() {
    let row = [keys("DeepSeek", "deepseek · not signed in")];
    assert_eq!(ranked("deeps", &row), [0]);
    assert!(ranked("signed", &row).is_empty(), "a note is not searched");
}
