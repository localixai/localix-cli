//! The native driver against fixture pages on a local server, in a real headless window.
//! The event loop must own the main thread, so this test has its own `main` (`harness = false`).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

use tools::browser::{self, Action, BrowserError, Driver, Observation, WebviewDriver, Window};

#[path = "support/panel.rs"]
mod panel;

/// Serves `tests/fixtures` on an ephemeral port; returns its origin. Each connection has its own
/// thread: a browser opens connections ahead of need, and one that never sends a request must not
/// hold up the next page.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || answer(&stream));
        }
    });
    origin
}

/// One request: the fixture it names, after the delay its path asks for.
fn answer(stream: &std::net::TcpStream) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .trim_start_matches('/');
    // A server that takes its time: a navigation must be followed, not raced.
    let (path, delay) = match (path.strip_prefix("slow/"), path.strip_prefix("slower/")) {
        (Some(rest), _) => (rest, 800),
        (_, Some(rest)) => (rest, 3000),
        _ => (path, 0),
    };
    std::thread::sleep(Duration::from_millis(delay));
    let file = format!("{}/tests/fixtures/{path}", env!("CARGO_MANIFEST_DIR"));
    let (status, body) = match std::fs::read(&file) {
        Ok(body) => ("200 OK", body),
        Err(_) => ("404 Not Found", b"missing".to_vec()),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut stream = stream;
    let _ = stream
        .write_all(head.as_bytes())
        .and_then(|_| stream.write_all(&body));
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
        .navigate(&std::env::var("BROWSER_DROPDOWN").unwrap_or(format!("{origin}/dropdown.html")))
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

    // A link inside a heading is a control in the outline, not only the heading's words.
    driver.navigate(&format!("{origin}/results.html")).unwrap();
    let page = driver.observe().unwrap();
    let title = find(&page, "link", "Cheap flights to Georgia");
    assert!(
        page.outline
            .contains(&format!("link \"Cheap flights to Georgia\" [ref={title}]"))
            && page.outline.contains("heading 2 \"Plain heading\""),
        "{}",
        page.outline
    );

    // Keys reach the focused element (here the body) with the key, code and keyCode games read.
    driver.navigate(&format!("{origin}/keys.html")).unwrap();
    driver
        .press(&["Up".into(), "ArrowLeft".into(), "a".into()], 0)
        .unwrap();
    assert_eq!(
        driver.evaluate("document.title").unwrap(),
        "keys:ArrowUp/ArrowUp/38;ArrowLeft/ArrowLeft/37;a/KeyA/65;"
    );
    // A script presses keys the same way, with `press` (a game bot's loop); a script may still
    // declare a `press` of its own.
    driver.navigate(&format!("{origin}/keys.html")).unwrap();
    assert_eq!(
        driver
            .evaluate("press('ArrowDown'); await sleep(10); press('b'); document.title")
            .unwrap(),
        "keys:ArrowDown/ArrowDown/40;b/KeyB/66;"
    );
    assert_eq!(driver.evaluate("const press = 2; press * 3").unwrap(), "6");

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
    // A modal hides the form from the controls; a password's value never shows.
    driver.navigate(&format!("{origin}/modal.html")).unwrap();
    let page = driver.observe().unwrap();
    let names: Vec<&str> = page.elements.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Done"]);
    assert!(
        !page.outline.contains("secret") && !page.text.contains("secret"),
        "{}",
        page.outline
    );
    driver.navigate(&format!("{origin}/dropdown.html")).unwrap();

    // Scripts: an expression, statements (last value), a body with its own return, a throw.
    assert_eq!(driver.evaluate("document.title").unwrap(), "Dropdown");
    assert_eq!(driver.evaluate("const n = 2; n * 21").unwrap(), "42");
    assert_eq!(
        driver
            .evaluate("let n = 0; while (n < 3) { n++ } n * 2")
            .unwrap(),
        "6"
    );
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

    // A script whose page leaves answers once the new document loads, not at the script timeout.
    driver.navigate(&format!("{origin}/form.html")).unwrap();
    let started = std::time::Instant::now();
    let left = driver
        .evaluate("location.href = '/second.html'; await new Promise((r) => setTimeout(r, 30000))")
        .unwrap();
    assert!(left.contains("new document"), "{left}");
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(driver.observe().unwrap().title, "Second");

    // Navigations only the Navigation API reports are still followed to their load.
    for button in ["Submit by script", "Go later"] {
        driver.navigate(&format!("{origin}/leave.html")).unwrap();
        let page = driver.observe().unwrap();
        act(
            driver,
            &page,
            Action::Click {
                reference: find(&page, "button", button).into(),
            },
        );
        assert_eq!(driver.observe().unwrap().title, "Second", "{button}");
    }

    // A parsed page is used after a short grace: a slow image (an ad, a video) does not hold the
    // run until it arrives, nor makes the page busy.
    let started = std::time::Instant::now();
    driver.navigate(&format!("{origin}/heavy.html")).unwrap();
    let page = driver.observe().unwrap();
    assert!(
        started.elapsed() < Duration::from_millis(2000),
        "{:?}",
        started.elapsed()
    );
    assert!(page.title == "Heavy" && !page.busy, "{page:?}");
    std::thread::sleep(Duration::from_millis(2500));

    // A page waiting for its own request is busy until the answer arrives.
    driver.navigate(&format!("{origin}/search.html")).unwrap();
    let page = driver.observe().unwrap();
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "button", "Search").into(),
        },
    );
    let page = driver.observe().unwrap();
    assert!(page.busy && !page.text.contains("3 results"), "{page:?}");
    std::thread::sleep(Duration::from_millis(1200));
    let page = driver.observe().unwrap();
    assert!(!page.busy && page.text.contains("3 results"), "{page:?}");

    // A banner in a same-origin frame over the page: its controls are offered as a dialog, and
    // typing and clicking reach them; the page under it refuses until it is gone.
    driver.navigate(&format!("{origin}/consent.html")).unwrap();
    let page = driver.observe().unwrap();
    let accept = find(&page, "button", "Accept").to_string();
    assert!(
        page.elements
            .iter()
            .any(|e| e.reference == accept && e.dialog),
        "{:#?}",
        page.elements
    );
    assert!(
        page.text.starts_with("We value your privacy"),
        "{}",
        page.text
    );
    let radio = find(&page, "radio", "<script>").to_string();
    let covered = driver.act(
        &page.epoch,
        &Action::Click {
            reference: radio.clone(),
        },
    );
    assert!(
        matches!(&covered, Err(BrowserError::Refused(m)) if m.starts_with("covered")),
        "{covered:?}"
    );
    act(
        driver,
        &page,
        Action::Type {
            reference: find(&page, "textbox", "Mail").into(),
            text: "a@b.c".into(),
        },
    );
    act(driver, &page, Action::Click { reference: accept });
    let page = driver.observe().unwrap();
    assert_eq!(page.title, "Quiz:a@b.c");
    act(driver, &page, Action::Click { reference: radio });
    let page = driver.observe().unwrap();
    let checked = page.elements.iter().find(|e| e.name == "<script>").unwrap();
    assert!(checked.states.contains(&"checked".into()), "{checked:?}");

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

    tabs(driver, origin);
    screenshots(driver, origin);

    // A dialog in an open shadow root (a consent web component): its controls are offered, as a
    // dialog; what it covers names it; its button closes it.
    driver.navigate(&format!("{origin}/shadow.html")).unwrap();
    let page = driver.observe().unwrap();
    let save = find(&page, "button", "Save + Exit").to_string();
    assert!(
        page.elements
            .iter()
            .any(|e| e.reference == save && e.dialog),
        "{:#?}",
        page.elements
    );
    let covered = driver.act(
        &page.epoch,
        &Action::Click {
            reference: find(&page, "button", "New Game").into(),
        },
    );
    assert!(
        matches!(&covered, Err(BrowserError::Refused(m))
            if m.starts_with("covered: a dialog \"Consent to Cookies\"")),
        "{covered:?}"
    );
    act(driver, &page, Action::Click { reference: save });
    let page = driver.observe().unwrap();
    assert!(page.elements.iter().all(|e| !e.dialog), "the dialog closed");
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "button", "New Game").into(),
        },
    );
    assert_eq!(driver.observe().unwrap().title, "new game");

    // Controls a page made from plain elements (a pointer cursor, an onclick) are offered and
    // clicked like buttons; a pointer group once, and a wrapper around a link not at all.
    driver.navigate(&format!("{origin}/levels.html")).unwrap();
    let page = driver.observe().unwrap();
    let names: Vec<(&str, &str)> = page
        .elements
        .iter()
        .map(|e| (e.role.as_str(), e.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("button", "Slug"),
            ("button", "Worm"),
            ("button", "Inline"),
            ("button", "Card title"),
            ("link", "More"),
            ("button", "sound toggle"),
        ]
    );
    let worm = find(&page, "button", "Worm").to_string();
    assert!(
        page.outline
            .contains(&format!("button \"Worm\" [ref={worm}]"))
            && page.outline.contains("text \"Plain words\""),
        "{}",
        page.outline
    );
    act(driver, &page, Action::Click { reference: worm });
    assert_eq!(driver.observe().unwrap().title, "level:Worm");

    // The agent closes the window: it is closed, and the task goes on (closing it by hand would
    // end this test with the window-closed exit code instead of "driver: ok").
    driver.close();
    let closing = std::time::Instant::now();
    while driver.is_open() && closing.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!driver.is_open(), "the window closed");
    assert!(matches!(
        driver.observe(),
        Err(BrowserError::Unavailable(_))
    ));
}

/// Tabs: one opened, switched to and back, one per link with target=_blank and per window.open,
/// a tab out of sight still drawing, and closing them.
fn tabs(driver: &WebviewDriver, origin: &str) {
    let titles = |driver: &WebviewDriver| {
        std::thread::sleep(Duration::from_millis(300));
        let (tabs, active) = driver.tabs();
        let titles: Vec<String> = tabs.iter().map(|tab| tab.title.clone()).collect();
        (titles, active)
    };
    driver.navigate(&format!("{origin}/tabs.html")).unwrap();
    assert_eq!(driver.tabs().0.len(), 1);
    let second = driver.new_tab(&format!("{origin}/form.html")).unwrap();
    assert_eq!(
        (second, driver.observe().unwrap().title.as_str()),
        (2, "Book")
    );
    assert_eq!(titles(driver), (vec!["Tabs".into(), "Book".into()], 2));

    // Tab 1 kept drawing while out of sight.
    driver.switch_tab(1).unwrap();
    let drawn = |driver: &WebviewDriver| {
        driver
            .evaluate("window.frames_drawn")
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    let before = drawn(driver);
    driver.switch_tab(2).unwrap();
    std::thread::sleep(Duration::from_millis(600));
    driver.switch_tab(1).unwrap();
    assert!(
        drawn(driver) > before + 10,
        "{before} then {}",
        drawn(driver)
    );

    // A link with target=_blank and window.open each open a tab, which is then acted on.
    let page = driver.observe().unwrap();
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "link", "Open second").into(),
        },
    );
    assert_eq!(driver.observe().unwrap().title, "Second");
    driver.switch_tab(1).unwrap();
    let page = driver.observe().unwrap();
    act(
        driver,
        &page,
        Action::Click {
            reference: find(&page, "button", "Open dropdown").into(),
        },
    );
    assert_eq!(driver.observe().unwrap().title, "Dropdown");
    assert_eq!(
        titles(driver),
        (
            vec![
                "Tabs".into(),
                "Book".into(),
                "Second".into(),
                "Dropdown".into()
            ],
            4
        )
    );

    // Closing the tab acted on goes to the one before it; a closed id is refused.
    assert!(driver.close_tab(4).unwrap());
    assert_eq!(driver.observe().unwrap().title, "Second");
    assert!(matches!(
        driver.switch_tab(4),
        Err(BrowserError::Refused(_))
    ));
    for id in [3, 2] {
        assert!(driver.close_tab(id).unwrap());
    }
    assert_eq!(titles(driver), (vec!["Tabs".into()], 1));
    assert_eq!(driver.observe().unwrap().title, "Tabs");
    // A new tab reuses a closed one's place.
    assert_eq!(driver.new_tab(&format!("{origin}/second.html")).unwrap(), 2);
    assert!(driver.close_tab(2).unwrap());
}

/// A screenshot is the WebView's own picture, as JPEG; a selector crops it to one element.
fn screenshots(driver: &WebviewDriver, origin: &str) {
    driver.navigate(&format!("{origin}/canvas.html")).unwrap();
    let viewport = driver.observe().unwrap().viewport;
    let page = driver.screenshot(None).unwrap();
    assert_eq!(&page.jpeg[..3], [0xFF, 0xD8, 0xFF], "a JPEG");
    let ratio = |w: u32, h: u32| f64::from(w) / f64::from(h);
    assert!(
        (ratio(page.width, page.height) - ratio(viewport.w, viewport.h)).abs() < 0.02
            && page.width.max(page.height) <= 1280
            && page.width >= viewport.w.min(1280) / 2,
        "{}x{} for a {}x{} viewport",
        page.width,
        page.height,
        viewport.w,
        viewport.h
    );
    // Below the fold: scrolled into view, then cut out with its 8 px margin (216x116).
    let board = driver.screenshot(Some("#board")).unwrap();
    assert!(
        (ratio(board.width, board.height) - 216.0 / 116.0).abs() < 0.05,
        "{}x{}",
        board.width,
        board.height
    );
    assert!(board.width < page.width && board.jpeg.len() < page.jpeg.len());
    let missing = driver.screenshot(Some("#nothing"));
    assert!(
        matches!(&missing, Err(BrowserError::Refused(m)) if m.contains("nothing on the page matches")),
        "{missing:?}"
    );
}

/// Two tests, `driver` and `panel`, as the standard harness lists and runs tests, so a test
/// runner that asks for the list (`--list --format terse`) and then runs each by name finds them.
/// The event loop takes the main thread once per process, so the chosen ones run in one loop.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--list") {
        if !args.iter().any(|arg| arg == "--ignored") {
            println!("driver: test\npanel: test");
        }
        return;
    }
    let filters: Vec<&String> = args.iter().filter(|arg| !arg.starts_with('-')).collect();
    let chosen =
        |name: &str| filters.is_empty() || filters.iter().any(|f| name.contains(f.as_str()));
    let (driving, paneling) = (chosen("driver"), chosen("panel"));
    if !driving && !paneling {
        return;
    }
    let origin = serve();
    let window = Window {
        url: format!("{origin}/form.html"),
        title: "browser driver test".into(),
        headless: true,
        viewport: browser::Viewport::DESKTOP,
    };
    // A window that cannot open ends the run with 3; the person closing it, with 130.
    browser::host(
        || std::process::exit(130),
        move |browser| {
            let run = |name: &str, test: &dyn Fn()| match catch_unwind(AssertUnwindSafe(test)) {
                Ok(()) => {
                    println!("{name}: ok");
                    true
                }
                Err(_) => false,
            };
            if paneling && !run("panel (first)", &|| panel::first(&browser)) {
                return 1;
            }
            let driver = match browser.open(window.clone()) {
                Ok(driver) => driver,
                Err(error) => {
                    eprintln!("{error}");
                    return 3;
                }
            };
            let driven = !driving || run("driver", &|| scenario(&driver, &origin));
            // The driver's scenario ends with its window closed: the panel's opens it again.
            let paneled = !paneling
                || run("panel", &|| {
                    let reopened = driver
                        .observe()
                        .is_err()
                        .then(|| browser.open(window.clone()));
                    let driver = match reopened {
                        Some(opened) => opened.expect("it opens again"),
                        None => driver.clone(),
                    };
                    panel::scenario(&browser, &driver);
                    panel::emulator(&browser);
                    panel::glass(&browser, &driver, &window, &origin);
                });
            if driven && paneled { 0 } else { 1 }
        },
    )
}
