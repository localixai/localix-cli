use std::cell::RefCell;

use super::*;

#[test]
fn a_number_picks_enter_takes_the_marked_and_q_cancels() {
    assert_eq!(chosen("2", 3, None), Answer::Chosen(1));
    assert_eq!(chosen(" ", 3, Some(2)), Answer::Chosen(2));
    assert_eq!(chosen("", 3, None), Answer::Again);
    assert_eq!(chosen("4", 3, None), Answer::Again);
    assert_eq!(chosen("0", 3, None), Answer::Again);
    assert_eq!(chosen("Q", 3, Some(0)), Answer::Cancel);
}

#[test]
fn the_list_is_numbered_and_a_wrong_answer_is_asked_again() {
    let screen = RefCell::new(String::new());
    let shown = |text: &str| screen.borrow_mut().push_str(text);
    let mut input: &[u8] = b"9\n2\n";
    let mut picker = Numbered {
        input: &mut input,
        shown: &shown,
    };
    let items = [Item::new("One", "first", true), Item::new("Two", "", false)];
    assert_eq!(picker.pick("Pick one", &items), Some(1));
    let screen = screen.borrow();
    assert!(
        screen.contains("Pick one\n  * 1. One  first\n    2. Two\n"),
        "{screen}"
    );
    assert!(screen.contains("Number (Enter: 1, q: cancel) > "));
    assert!(screen.contains("1 to 2, or q"));
}

#[test]
fn the_end_of_input_cancels() {
    let shown = |_: &str| {};
    let mut input: &[u8] = b"";
    let mut picker = Numbered {
        input: &mut input,
        shown: &shown,
    };
    assert_eq!(picker.pick("Pick", &[Item::new("One", "", false)]), None);
    assert_eq!(picker.line("> ", false), None);
}
