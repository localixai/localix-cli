use super::*;

#[test]
fn a_form_encodes_everything_but_unreserved_characters() {
    assert_eq!(
        form(&[
            ("grant_type", "authorization_code"),
            ("redirect_uri", "http://127.0.0.1:1455/a b")
        ]),
        "grant_type=authorization_code&redirect_uri=http%3A%2F%2F127.0.0.1%3A1455%2Fa%20b"
    );
}

#[test]
fn a_query_value_is_found_and_decoded() {
    let target = "/auth/callback?code=ab%2Fc&state=x+y#frag";
    assert_eq!(query_value(target, "code"), Some("ab/c".into()));
    assert_eq!(query_value(target, "state"), Some("x y".into()));
    assert_eq!(query_value(target, "error"), None);
    assert_eq!(query_value("/no-query", "code"), None);
}

#[test]
fn a_broken_escape_stays_as_it_was() {
    assert_eq!(decode("100%"), "100%");
    assert_eq!(decode("%zz%4"), "%zz%4");
    assert_eq!(decode("%C3%A9"), "\u{e9}");
}
