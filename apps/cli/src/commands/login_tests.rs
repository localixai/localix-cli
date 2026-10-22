use super::*;

#[test]
fn a_key_shows_only_its_prefix_and_its_last_four() {
    assert_eq!(mask("sk-or-v1-0123456789abcdef"), "sk-or-...cdef");
}

#[test]
fn a_short_key_shows_nothing_of_itself() {
    assert_eq!(mask("sk-or-abc"), "...");
    assert_eq!(mask(""), "...");
}
