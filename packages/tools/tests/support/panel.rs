//! A panel as a tab of the one window, beside the browser's tabs: a terminal session's page as
//! the shell makes it, painted from this thread, read back from its page.

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tools::browser::{Browser, Driver, Panel, WebviewDriver, Window};

/// What the page shows: the first rows' text, the cursor, the title, and whether it has the
/// keyboard.
const READ: &str = "(() => { const c = document.getElementById('c'); return { \
    rows: [...document.querySelectorAll('.r')].slice(0, 3).map(r => r.textContent), \
    top: c.style.top, left: c.style.left, hidden: c.hidden, title: document.title, \
    focus: document.hasFocus(), href: location.href, p: typeof window.p, \
    markup: document.querySelectorAll('#s img, #s script').length, width: innerWidth, \
    height: innerHeight, cells: typeof cells === 'function' ? cells() : null, visible: document.visibilityState, ground: getComputedStyle(document.documentElement).backgroundColor }; \
    })()";

fn frame(lines: &[String], row: u16, column: u16, end: Option<&str>) -> String {
    let mut rows = lines.to_vec();
    rows.resize(40, String::new());
    let rows = serde_json::to_string(&rows)
        .unwrap()
        .replace('<', "\\u003c");
    format!("F({rows},{row},{column},false,{})", json!(end))
}

fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(|line| line.to_string()).collect()
}

/// The tab the window shows, and every tab's id, as the window lists them.
fn showing(browser: &Browser) -> Option<(String, Vec<String>)> {
    let listed = browser.listing()?;
    let shown = listed
        .iter()
        .find(|tab| tab.shown)
        .map(|tab| tab.id.clone());
    Some((
        shown.unwrap_or_default(),
        listed.into_iter().map(|tab| tab.id).collect(),
    ))
}

/// The page once `done` holds of it, within `within`.
fn shown(
    browser: &Browser,
    panel: &Panel,
    within: Duration,
    done: impl Fn(&Value) -> bool,
) -> Value {
    let deadline = Instant::now() + within;
    loop {
        let read = browser.peek(panel, READ).unwrap_or(Value::Null);
        if done(&read) {
            return read;
        }
        assert!(Instant::now() < deadline, "the panel shows {read:#}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn scenario(browser: &Browser, driver: &WebviewDriver) {
    let page = tools::shell::page("session 1 · cat", None, None);
    let panel = browser.panel(1, "session 1 · cat", page, None);
    let second = Duration::from_secs(1);
    let wait = Duration::from_secs(10);

    // The rows and the cursor as painted, and the window never took the keyboard.
    assert!(panel.paint(frame(&lines(&["hi", "there"]), 1, 5, None)));
    let read = shown(browser, &panel, wait, |read| read["rows"][1] == "there");
    assert_eq!(read["rows"], json!(["hi", "there", ""]));
    // The engine writes a calc() its own way: the cursor is 5 columns and 1 row in.
    let (left, top) = (read["left"].to_string(), read["top"].to_string());
    assert!(left.contains(" 5ch") || left.contains("(5ch"), "{left}");
    assert!(top.contains("1.2em"), "{top}");
    assert_eq!(read["hidden"], false);
    assert_eq!(read["focus"], false, "the panel took the keyboard");
    // Drawn as a shown page is, with animation frames: an emulator draws in them.
    assert_eq!(read["visible"], "visible");
    // The page fills its window (960 by 700 points, less the title bar).
    let (width, height) = (read["width"].as_f64(), read["height"].as_f64());
    assert!(
        width >= Some(900.0) && height >= Some(600.0),
        "{width:?} by {height:?}"
    );
    let href = read["href"].clone();

    // A tab beside the browser's, shown as it opens.
    let tab = format!("t{}", panel.id());
    let (shown_now, keys) = showing(browser).expect("the window is open");
    assert_eq!(
        (shown_now.as_str(), keys.clone()),
        (tab.as_str(), vec!["w1".into(), tab.clone()])
    );

    // What a program prints shows as text: no markup, no script, nothing run.
    let fixture = format!(
        "{}/tests/fixtures/panel-hostile.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let hostile: Vec<String> = serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
    assert!(panel.paint(frame(&hostile, 0, 0, None)));
    let read = shown(browser, &panel, second, |read| {
        read["rows"][0] == hostile[0]
    });
    assert_eq!(read["rows"], json!(hostile));
    assert_eq!(read["markup"], 0);
    assert_eq!(read["p"], "undefined", "a row ran as script");

    // The page goes nowhere else, whatever it is told.
    let _ = browser.peek(&panel, "location.href = 'https://example.com'; 1");
    std::thread::sleep(Duration::from_millis(500));
    let read = shown(browser, &panel, second, |read| read["href"] != Value::Null);
    assert_eq!(read["href"], href, "the panel navigated");

    // A burst: the newest frame within a second, and the browser still answers.
    for n in 0..1000 {
        panel.paint(frame(&lines(&[&n.to_string()]), 0, 0, None));
    }
    shown(browser, &panel, second, |read| read["rows"][0] == "999");
    driver.observe().expect("the browser answers after a burst");

    // A session's end, in the title.
    panel.paint(frame(&lines(&["bye"]), 0, 3, Some(" · exited 3")));
    shown(browser, &panel, second, |read| {
        read["title"] == "session 1 · cat · exited 3"
    });

    // What the agent does in the browser is shown; a click on the strip shows the terminal again.
    let until_shown = |want: &str| {
        let deadline = Instant::now() + wait;
        while showing(browser).map(|s| s.0).as_deref() != Some(want) {
            assert!(Instant::now() < deadline, "{:?}", showing(browser));
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    driver.navigate("about:blank").ok();
    until_shown("w1");
    browser.click(&format!("show {tab}"));
    until_shown(&tab);
    // The agent shows a tab too, and is told when there is none.
    browser.act("w1", false).unwrap();
    until_shown("w1");
    let listed = browser.listing().unwrap();
    assert_eq!((listed[0].kind, listed[0].acted_on), ("browser", true));
    assert_eq!(listed[1].kind, "terminal");
    assert!(listed[1].title.starts_with("session 1 · cat"), "{listed:?}");
    let missing = browser.act("t999", false).unwrap_err();
    assert!(
        missing.starts_with("there is no tab t999: the tabs are w1, t"),
        "{missing}"
    );
    browser.act(&tab, false).unwrap();
    until_shown(&tab);
    // Junk from the strip does nothing.
    browser.click("show t999");
    browser.click("<script>");
    until_shown(&tab);
    // Its × closes the tab: the caller hears so, and so does whoever listens to the window.
    let heard = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
    let told = heard.clone();
    browser.listen(move |key, what| {
        told.lock()
            .unwrap()
            .push((key.to_string(), what.to_string()))
    });
    browser.click(&format!("close {tab}"));
    until_shown("w1");
    let deadline = Instant::now() + wait;
    while panel.paint(frame(&lines(&["after"]), 0, 0, None)) {
        assert!(Instant::now() < deadline, "the panel still says it is open");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(showing(browser).map(|s| s.1), Some(vec!["w1".to_string()]));
    let heard = heard.lock().unwrap().clone();
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert_eq!(heard[0].0, tab);
    assert!(heard[0].1.contains("its program runs on"), "{heard:?}");

    // Gone, and the browser carries on.
    drop(panel);
    std::thread::sleep(Duration::from_millis(200));
    driver
        .observe()
        .expect("the browser answers after a panel closed");
}

/// The shown browser window: glass on macOS behind its titlebar, its page the viewport exactly,
/// and its pages opaque: a clear page and a white one make the same screenshot.
pub fn glass(browser: &Browser, driver: &WebviewDriver, window: &Window, origin: &str) {
    driver.close();
    let shown = Window {
        url: format!("{origin}/blank-white.html"),
        headless: false,
        ..window.clone()
    };
    let driver = browser.open(shown).expect("a shown window opens");
    driver.wait_loaded(Duration::from_secs(10)).unwrap();
    // A page there runs its animation frames, as in any shown window.
    let state = driver.evaluate("document.visibilityState").unwrap();
    assert!(
        state.contains("visible") && !state.contains("hidden"),
        "{state}"
    );
    // Coated as a panel is, so its titlebar and toolbar are not the bare material's grey.
    let expected = if cfg!(target_os = "macos") {
        (1, 1, false, true)
    } else {
        (0, -1, true, false)
    };
    let dressed = browser.dressed().map(|d| (d.0, d.1, d.2, d.4));
    assert_eq!(dressed, Some(expected), "the browser's frame");
    let size = driver.observe().unwrap().viewport;
    // A shown window fits the person's screen (960 to 1440 wide), the strip of tabs above it.
    assert!(
        (960..=1440).contains(&size.w) && (600..=930).contains(&size.h),
        "{} by {}",
        size.w,
        size.h
    );
    let white = driver.screenshot(None).unwrap();
    driver
        .navigate(&format!("{origin}/blank-transparent.html"))
        .unwrap();
    driver.wait_loaded(Duration::from_secs(10)).unwrap();
    assert_eq!(driver.observe().unwrap().title, "clear");
    let clear = driver.screenshot(None).unwrap();
    assert!(
        clear.jpeg == white.jpeg,
        "a clear page shows what is behind it"
    );
    driver.close();
}

fn asset(name: &str, honest: bool) -> tools::shell::Asset {
    use base64::Engine;
    use sha2::{Digest, Sha384};
    let file = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(file).unwrap();
    let hashed = if honest {
        text.clone()
    } else {
        format!("{text} ")
    };
    let sha384 = base64::engine::general_purpose::STANDARD.encode(Sha384::digest(&hashed));
    tools::shell::Asset { text, sha384 }
}

/// The emulator's page runs the scripts it was given by their hashes, and nothing whose hash is
/// not the one given: an emulator that was not verified never runs.
pub fn emulator(browser: &Browser) {
    let read = "({ made: JSON.parse(JSON.stringify(window.made || null)), drawn: window.drawn || '', title: document.title, \
        opened: window.opened || false })";
    let wait = Duration::from_secs(10);
    for honest in [true, false] {
        let emulator = tools::shell::Emulator {
            script: asset("emulator-fake.js", honest),
            fit: asset("emulator-fit-fake.js", honest),
            css: String::new(),
        };
        let page = tools::shell::page("session 3 · sh", None, Some(&emulator));
        let panel = browser.panel(3, "session 3 · sh", page, None);
        let mut rows = vec!["$ ls".to_string()];
        rows.resize(40, String::new());
        let drawn = "\u{1b}[?25h\u{1b}[m\u{1b}[H\u{1b}[J$ ls";
        let frame = format!("F({},0,4,false,null,{})", json!(rows), json!(drawn));
        assert!(panel.paint(frame));
        let deadline = std::time::Instant::now() + wait;
        let page = loop {
            let page = browser.peek(&panel, read).unwrap_or(Value::Null);
            if !honest && page["title"] == "session 3 · sh" || page["drawn"].as_str() > Some("") {
                break page;
            }
            assert!(std::time::Instant::now() < deadline, "{page:#}");
            std::thread::sleep(Duration::from_millis(20));
        };
        if honest {
            assert_eq!(page["made"]["cols"], 120, "{page:#}");
            assert_eq!(page["made"]["rows"], 40);
            assert_eq!(page["made"]["disableStdin"], true);
            assert_eq!(page["opened"], true);
            assert!(
                page["drawn"].as_str().unwrap().ends_with("$ ls"),
                "{page:#}"
            );
        } else {
            std::thread::sleep(Duration::from_millis(300));
            let page = browser.peek(&panel, read).unwrap();
            assert_eq!((&page["made"], &page["drawn"]), (&Value::Null, &json!("")));
        }
    }
}

/// A panel opened before any browser window draws as a shown page does, with animation frames
/// (an emulator draws in them): WebKit must not take it for a hidden one.
pub fn first(browser: &Browser) {
    let page = tools::shell::page("session 0 · sh", None, None);
    let panel = browser.panel(9, "session 0 · sh", page, None);
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while browser.peek(&panel, "typeof F") != Ok(json!("function")) {
        assert!(
            std::time::Instant::now() < deadline,
            "the panel never loaded"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    // The window opens for it alone: its one tab, shown.
    let tab = format!("t{}", panel.id());
    assert_eq!(showing(browser), Some((tab.clone(), vec![tab])));
    // Long enough for WebKit to have settled what it thinks of the window.
    std::thread::sleep(Duration::from_millis(300));
    let count = "window.seen = 0; (function f() { window.seen++; requestAnimationFrame(f); })(); 1";
    browser.peek(&panel, count).unwrap();
    let read = "[window.seen, document.visibilityState]";
    std::thread::sleep(Duration::from_millis(500));
    let seen = browser.peek(&panel, read).unwrap();
    assert_eq!(seen[1], "visible", "{seen}");
    assert!(seen[0].as_u64() > Some(5), "no animation frames: {seen}");
    // The last tab gone, the window is too.
    drop(panel);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while showing(browser).is_some() {
        assert!(std::time::Instant::now() < deadline, "the window stayed");
        std::thread::sleep(Duration::from_millis(50));
    }
}
