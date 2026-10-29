use std::net::TcpListener;

use super::*;

const APP: App = App {
    name: "llx",
    slug: "llx",
    version: "0",
};

#[test]
fn a_request_refused_was_not_sent_and_one_left_unanswered_was() {
    // Nothing listens at a port just let go: the request does not get through.
    let closed = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let web = Web::new(APP, Duration::from_millis(300));
    let refused = web.post(&format!("http://{closed}/x"), &[], "{}", None);
    assert!(matches!(refused, Err(HttpError::Unsent(_))), "{refused:?}");
    // A server that takes the request and says nothing until the time runs out.
    let silent = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = silent.local_addr().unwrap();
    let unanswered = web.post(&format!("http://{address}/x"), &[], "{}", None);
    assert!(
        matches!(unanswered, Err(HttpError::Unanswered(_))),
        "{unanswered:?}"
    );
    drop(silent);
}

#[test]
fn a_connection_closed_after_the_request_went_out_is_unanswered() {
    // A server that reads the request and hangs up without a word: it may have acted on it.
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = server.local_addr().unwrap();
    let hangup = std::thread::Builder::new()
        .name("hangup".into())
        .spawn(move || {
            let (mut socket, _) = server.accept().unwrap();
            let mut seen = [0_u8; 4096];
            let _ = std::io::Read::read(&mut socket, &mut seen);
        })
        .unwrap();
    let web = Web::new(APP, Duration::from_secs(5));
    let reply = web.post(&format!("http://{address}/x"), &[], "{}", None);
    hangup.join().unwrap();
    assert!(matches!(reply, Err(HttpError::Unanswered(_))), "{reply:?}");
}

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

#[test]
fn a_deadline_ends_the_wait_for_an_answer() {
    // A server that takes the connection and holds it, saying nothing.
    let silent = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = silent.local_addr().unwrap();
    let web = Web::new(APP, Duration::from_secs(30));
    let started = Instant::now();
    let reply = web.post(
        &format!("http://{address}/x"),
        &[],
        "{}",
        Some(started + Duration::from_millis(200)),
    );
    assert!(matches!(reply, Err(HttpError::Unanswered(_))), "{reply:?}");
    assert!(started.elapsed() < Duration::from_secs(2));
    drop(silent);
}

#[test]
fn a_deadline_already_passed_sends_nothing() {
    let web = Web::new(APP, Duration::from_secs(30));
    let reply = web.post("http://127.0.0.1:9/x", &[], "{}", Some(Instant::now()));
    assert_eq!(reply, Err(HttpError::Unsent(OUT_OF_TIME.into())));
}
