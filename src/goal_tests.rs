use super::*;

#[test]
fn a_loop_is_a_state_seen_again_not_a_repeated_action() {
    // Open search, close it, open it again: the third visit to the same state is a loop.
    let (closed, open) = (1, 2);
    assert!(!cycling(&[closed, open], closed));
    assert!(cycling(&[closed, open, closed, open], closed));
    // "Scroll right" five times, each showing new dates: never a loop.
    assert!(!cycling(&[10, 11, 12, 13], 14));
}

#[test]
fn origin_is_scheme_host_and_port() {
    assert_eq!(
        origin("https://duckduckgo.com/?q=x"),
        "https://duckduckgo.com"
    );
    assert_eq!(origin("http://127.0.0.1:8080/a#b"), "http://127.0.0.1:8080");
    assert_eq!(
        origin("https://docs.typesafe.ai"),
        "https://docs.typesafe.ai"
    );
}
