use serde_json::json;

use super::*;

#[test]
fn a_long_argument_reads_on_one_short_line() {
    assert_eq!(one_line("a\n  b\tc", 10), "a b c");
    assert_eq!(one_line("abcdef", 3), "abc…");
}

#[test]
fn a_call_reads_as_the_arguments_it_was_given() {
    let args =
        json!({"goal": "Search", "values": ["Tbilisi"], "keys": [], "read": false, "url": ""});
    assert_eq!(
        call_line("browser", &args),
        "▸ browser goal: Search · values: [\"Tbilisi\"]"
    );
}
