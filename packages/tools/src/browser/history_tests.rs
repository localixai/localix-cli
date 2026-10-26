use std::collections::HashMap;

use super::*;

fn none() -> HashMap<String, String> {
    HashMap::new()
}

fn listed(history: &History) -> Vec<(String, String, bool)> {
    let list = history.list().into_iter();
    list.map(|e| (e.key.name(), e.detail, e.closed)).collect()
}

#[test]
fn a_tab_is_one_entry_with_its_last_page_the_newest_first() {
    let history = History::default();
    history.web(&[1], &none());
    history.loaded(1, "https://a.example/");
    history.loaded(1, "https://b.example/");
    history.web(&[1, 2], &none());
    history.loaded(2, "https://c.example/");
    history.titled(Key::Web(1), "  B  ");
    assert_eq!(
        listed(&history),
        [
            ("w2".into(), "https://c.example/".into(), false),
            ("w1".into(), "https://b.example/".into(), false),
        ]
    );
    assert_eq!(history.list()[1].title, "B");
}

#[test]
fn a_closed_tab_stays_and_its_id_used_again_is_another_entry() {
    let history = History::default();
    history.web(&[1, 2], &none());
    history.loaded(2, "https://x.example/");
    let groups = HashMap::from([("w2".to_string(), "shop".to_string())]);
    history.web(&[1], &groups);
    history.web(&[1, 2], &none());
    let list = history.list();
    let twos: Vec<_> = list.iter().filter(|e| e.key == Key::Web(2)).collect();
    assert_eq!(twos.len(), 2, "{list:?}");
    assert!(!twos[0].closed && twos[1].closed, "{list:?}");
    assert_eq!(twos[1].detail, "https://x.example/");
    // The closed one keeps the project it was of, though its key is another tab's now.
    assert_eq!(twos[1].group, "shop");
    assert_ne!(twos[0].at, twos[1].at);
    assert_eq!(
        history.get(twos[1].at).map(|e| e.detail),
        Some(twos[1].detail.clone())
    );
}

#[test]
fn a_closed_tab_that_never_showed_a_web_page_is_not_kept() {
    let history = History::default();
    history.web(&[1, 2], &none());
    history.loaded(1, "https://a.example/");
    history.loaded(2, "about:blank");
    history.loaded(2, "data:text/html,x");
    history.web(&[], &none());
    assert_eq!(
        listed(&history),
        [("w1".into(), "https://a.example/".into(), true)]
    );
}

#[test]
fn a_terminal_is_an_entry_until_its_id_opens_another() {
    let history = History::default();
    history.opened(Key::Term(3), "session 3 · sh");
    history.titled(Key::Term(3), "session 3 · sh · exited 0");
    let first = history.list()[0].at;
    assert_eq!(history.list()[0].title, "session 3 · sh · exited 0");
    history.opened(Key::Term(3), "session 3 · python3");
    let list = history.list();
    assert_eq!(list.len(), 1, "{list:?}");
    assert_ne!(list[0].at, first);
    assert_eq!(list[0].title, "session 3 · python3");
}

#[test]
fn only_so_many_are_kept() {
    let history = History::default();
    let past = u32::try_from(KEEP).unwrap() + 5;
    for n in 0..past {
        history.opened(Key::Term(n), "sh");
    }
    let list = history.list();
    assert_eq!(list.len(), KEEP);
    assert_eq!(list[0].key, Key::Term(past - 1));
}
