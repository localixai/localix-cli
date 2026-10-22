use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

use super::*;
use crate::picker::{Item, Lines, Picker};

/// A person who types lines from a script.
struct Typed(VecDeque<&'static str>);

impl Picker for Typed {
    fn pick(&mut self, _: &str, _: &[Item]) -> Option<usize> {
        None
    }
}

impl Lines for Typed {
    fn line(&mut self, _: &str, _: bool) -> Option<String> {
        self.0.pop_front().map(str::to_string)
    }
}

/// `come_back` with lines typed, the callback coming back on the `back`-th wait.
fn signed(lines: &[&'static str], back: usize) -> (Result<String, String>, String) {
    let mut terminal = Typed(lines.iter().copied().collect());
    let said = RefCell::new(String::new());
    let waits = Cell::new(0);
    let wait = |_| {
        waits.set(waits.get() + 1);
        (waits.get() == back).then(|| Ok("from-browser".to_string()))
    };
    let pasted = |text: &str| Ok(format!("pasted:{text}"));
    let say = |text: &str| said.borrow_mut().push_str(text);
    let outcome = come_back(&mut terminal, &say, &wait, &pasted);
    (outcome, said.into_inner())
}

#[test]
fn enter_waits_for_the_browser_and_says_when_it_has_not_come_back() {
    let (outcome, said) = signed(&["", ""], 2);
    assert_eq!(outcome, Ok("from-browser".into()));
    assert!(said.contains("has not come back yet"), "{said}");
}

#[test]
fn a_pasted_address_is_read_and_q_or_the_end_of_input_cancels() {
    assert_eq!(
        signed(&["http://x/?code=c"], 0).0,
        Ok("pasted:http://x/?code=c".into())
    );
    assert_eq!(signed(&["q"], 0).0, Err("cancelled".into()));
    assert_eq!(signed(&[], 0).0, Err("cancelled".into()));
}

#[test]
fn a_key_shows_only_its_prefix_and_its_last_four() {
    assert_eq!(mask("sk-or-v1-0123456789abcdef"), "sk-or-...cdef");
    assert_eq!(mask("sk-or-abc"), "...");
}
