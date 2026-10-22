//! The native driver against fixture pages on a local server, in a real headless window.
//! The event loop must own the main thread, so this test has its own `main` (`harness = false`).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

use localix_surf::browser::{
    self, Action, BrowserError, Driver, Observation, WebviewDriver, Window,
};

/// Serves `tests/fixtures` on an ephemeral port; returns its origin.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            let path = line
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .trim_start_matches('/');
            let file = format!("{}/tests/fixtures/{path}", env!("CARGO_MANIFEST_DIR"));
            let (status, body) = match std::fs::read(&file) {
                Ok(body) => ("200 OK", body),
                Err(_) => ("404 Not Found", b"missing".to_vec()),
            };
            let mut stream = &stream;
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream
                .write_all(head.as_bytes())
                .and_then(|_| stream.write_all(&body));
        }
    });
    origin
}

fn find<'a>(page: &'a Observation, role: &str, name: &str) -> &'a str {
    page.elements
        .iter()
        .find(|element| element.role == role && element.name == name)
        .unwrap_or_else(|| panic!("no {role} \"{name}\" in {:#?}", page.elements))
        .reference
        .as_str()
}

fn act(driver: &WebviewDriver, page: &Observation, action: Action) {
    driver
        .act(&page.epoch, &action)
        .unwrap_or_else(|error| panic!("{action:?}: {error}"));
}

fn scenario(driver: &WebviewDriver, origin: &str) {
    driver.wait_loaded(Duration::from_secs(10)).unwrap();
    let page = driver.observe().unwrap();
    assert_eq!(page.title, "Book");
    let accept = find(&page, "button", "Accept");
    assert!(
        page.elements
            .iter()
            .any(|e| e.reference == accept && e.dialog),
        "Accept is in the dialog"
    );
    let later = page.elements.iter().find(|e| e.name == "Later").unwrap();
    assert!(later.disabled(), "a disabled control reads as disabled");

    // A disabled control refuses; a click closes the dialog.
    let refused = driver.act(
        &page.epoch,
        &Action::Click {
            reference: later.reference.clone(),
        },
    );
    assert!(
        matches!(refused, Err(BrowserError::Refused(_))),
        "{refused:?}"
    );
    act(
        driver,
        &page,
        Action::Click {
            reference: accept.to_string(),
        },
    );
    let page = driver.observe().unwrap();
    assert!(
        page.elements.iter().all(|e| e.name != "Accept"),
        "the dialog closed"
    );

    // Typing, picking an option and Enter reach the form's own submit handler.
    let city = find(&page, "textbox", "City").to_string();
    act(
        driver,
        &page,
        Action::Type {
            reference: city.clone(),
            text: "Zurich".into(),
        },
    );
    act(
        driver,
        &page,
        Action::Select {
            reference: find(&page, "combobox", "Class").into(),
            value: "Business".into(),
        },
    );
    act(
        driver,
        &page,
        Action::PressEnter {
            reference: Some(city),
        },
    );
    let page = driver.observe().unwrap();
    assert_eq!(page.title, "sent:Zurich:Business");
    let field = page.elements.iter().find(|e| e.name == "City").unwrap();
    assert_eq!(field.value.as_deref(), Some("Zurich"));

    // An unknown ref is stale; a covered control refuses rather than clicking what is on top.
    let unknown = driver.act(
        &page.epoch,
        &Action::Type {
            reference: "e999".into(),
            text: "x".into(),
        },
    );
    assert!(
        matches!(unknown, Err(BrowserError::Stale(_))),
        "{unknown:?}"
    );
    let veiled = find(&page, "button", "Under the veil").to_string();
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "button", "Raise veil").into(),
        },
    );
    let covered = driver.act(&page.epoch, &Action::Click { reference: veiled });
    assert!(
        matches!(&covered, Err(BrowserError::Refused(m)) if m.starts_with("covered")),
        "{covered:?}"
    );
    assert_ne!(driver.observe().unwrap().title, "veiled clicked");

    // Picking the chosen option again fires no change: the page's handler (as on
    // the-internet.herokuapp.com/dropdown) would otherwise reset the selection.
    driver
        .navigate(&std::env::var("LXSURF_DROPDOWN").unwrap_or(format!("{origin}/dropdown.html")))
        .unwrap();
    for _ in 0..3 {
        let page = driver.observe().unwrap();
        let reference = page.elements[0].reference.clone();
        act(
            driver,
            &page,
            Action::Select {
                reference,
                value: "2".into(),
            },
        );
        let after = driver.observe().unwrap();
        assert_eq!(
            after.elements[0].value.as_deref(),
            Some("Option 2"),
            "{:?}",
            after.elements[0]
        );
    }

    // Calendar days carry their date or month: "5" alone is ambiguous across two months.
    driver.navigate(&format!("{origin}/calendar.html")).unwrap();
    let days: Vec<String> = driver
        .observe()
        .unwrap()
        .elements
        .iter()
        .map(|e| e.name.clone())
        .collect();
    assert_eq!(days, ["5 (2026-12-05)", "5 €120 (January 2027)"]);
    // Keys reach the page as a game reads them.
    driver.navigate(&format!("{origin}/keys.html")).unwrap();
    driver
        .press(&["Up".into(), "ArrowLeft".into(), "a".into()], 0)
        .unwrap();
    assert_eq!(
        driver.observe().unwrap().title,
        "keys:ArrowUp/ArrowUp/38;ArrowLeft/ArrowLeft/37;a/KeyA/65;"
    );

    // A modal hides the form from the controls, not its values; a password never shows.
    driver.navigate(&format!("{origin}/modal.html")).unwrap();
    let page = driver.observe().unwrap();
    let names: Vec<&str> = page.elements.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Done"]);
    let fields: Vec<(&str, &str)> = page
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.value.as_str()))
        .collect();
    assert_eq!(fields, [("Where from?", "Larnaca")]);
    driver.navigate(&format!("{origin}/dropdown.html")).unwrap();

    // Scripts: an expression, statements (last value), a body with its own return, a throw.
    assert_eq!(driver.evaluate("document.title").unwrap(), "Dropdown");
    assert_eq!(driver.evaluate("const n = 2; n * 21").unwrap(), "42");
    assert_eq!(
        driver
            .evaluate("const xs = [1, 2]; return xs.length")
            .unwrap(),
        "2"
    );
    assert_eq!(
        driver.evaluate("await Promise.resolve({a: 1})").unwrap(),
        "{\n  \"a\": 1\n}"
    );
    let thrown = driver.evaluate("null.x");
    assert!(
        matches!(&thrown, Err(BrowserError::Refused(m)) if m.starts_with("the script threw")),
        "{thrown:?}"
    );

    // A link navigates; the old epoch no longer acts on the new document.
    driver.navigate(&format!("{origin}/form.html")).unwrap();
    let page = driver.observe().unwrap();
    let old = page.epoch.clone();
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "link", "Next page").into(),
        },
    );
    let page = driver.observe().unwrap();
    assert_eq!(page.title, "Second");
    let back = find(&page, "link", "Back").to_string();
    let stale = driver.act(&old, &Action::Click { reference: back });
    assert!(matches!(stale, Err(BrowserError::Stale(_))), "{stale:?}");
}

fn main() {
    let origin = serve();
    let window = Window {
        url: format!("{origin}/form.html"),
        title: "lxsurf driver test".into(),
        headless: true,
        viewport: browser::Viewport::DESKTOP,
    };
    browser::run(window, move |driver| {
        match catch_unwind(AssertUnwindSafe(|| scenario(&driver, &origin))) {
            Ok(()) => {
                println!("driver: ok");
                0
            }
            Err(_) => 1,
        }
    })
}
