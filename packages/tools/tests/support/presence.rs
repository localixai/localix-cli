//! The window's three states (ARCHITECTURE.md A9), through real windows (unseen, as every test's):
//! a shown window opens as the mini, the agent's acts never expand it, collapsing never reflows
//! the page nor stops it drawing, and llx is never the active app.

use std::time::{Duration, Instant};

use serde_json::json;
use tools::browser::{Browser, Driver, WebviewDriver, Window};

/// Until `want` holds, within ten seconds.
fn until(what: &str, want: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !want() {
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The page's frame count over `span`: it rises while the window keeps drawing.
fn frames(driver: &WebviewDriver, span: Duration) -> (u64, u64) {
    let count = "window.seen = 0; (function f() { window.seen++; requestAnimationFrame(f); })(); 1";
    driver.evaluate(count).unwrap();
    let read = || driver.evaluate("window.seen").unwrap().parse().unwrap();
    let first = read();
    std::thread::sleep(span);
    (first, read())
}

/// The page's size as the agent reads it.
fn viewport(driver: &WebviewDriver) -> (u32, u32) {
    let size = driver.observe().unwrap().viewport;
    (size.w, size.h)
}

/// The mini's picture's mean brightness, 0 to 255; -1 while it has none.
const BRIGHTNESS: &str = "(() => { const i = document.getElementById('p'); \
    if (!i.complete || !i.naturalWidth) return -1; \
    const c = document.createElement('canvas'); c.width = 8; c.height = 8; \
    const x = c.getContext('2d'); x.drawImage(i, 0, 0, 8, 8); \
    const d = x.getImageData(0, 0, 8, 8).data; let s = 0; \
    for (let k = 0; k < d.length; k += 4) s += d[k] + d[k + 1] + d[k + 2]; \
    return s / (d.length / 4) / 3; })()";

pub(crate) fn scenario(browser: &Browser, window: &Window, origin: &str) {
    let shown = Window {
        url: format!("{origin}/slow/light.html"),
        headless: false,
        ..window.clone()
    };
    assert_eq!(browser.presented(), Some("hidden"), "nothing is open yet");
    let driver = browser.open(shown).expect("a shown window opens");
    driver.wait_loaded(Duration::from_secs(10)).unwrap();

    // The first tab opens as the mini, and the window behind it keeps drawing.
    until("the mini", || browser.presented() == Some("mini"));
    let (first, later) = frames(&driver, Duration::from_millis(500));
    assert!(
        later > first + 5,
        "the page stopped drawing: {first} to {later}"
    );
    let size = viewport(&driver);
    assert!(size.0 >= 600 && size.1 >= 400, "{size:?}");

    // The mini has the tab's picture and name, and no spinner once the agent is done.
    let named = "document.getElementById('i').textContent";
    until("the tab's id on the mini", || {
        browser.peek_mini(named) == Ok(json!("w1"))
    });
    until("a picture on the mini", || {
        let src = browser.peek_mini("document.getElementById('p').src");
        src.is_ok_and(|src| {
            src.as_str()
                .is_some_and(|s| s.starts_with("data:image/jpeg"))
        })
    });

    // The picture is the loaded page, not the blank tab it was before: light, as the page is.
    until("the loaded page on the mini", || {
        browser
            .peek_mini(BRIGHTNESS)
            .is_ok_and(|seen| seen.as_f64().is_some_and(|b| b > 200.0))
    });

    // The agent shows a tab: the mini blinks and stays the mini.
    browser.act("w1", false).unwrap();
    assert_eq!(browser.presented(), Some("mini"));
    // Nor does anything else it does expand it.
    driver.navigate(&format!("{origin}/form.html")).unwrap();
    assert_eq!(browser.presented(), Some("mini"));

    // The person's click expands it; the page is as big as it was, and still drawing.
    browser.click_mini();
    until("the expanded window", || {
        browser.presented() == Some("expanded")
    });
    assert_eq!(viewport(&driver), size, "expanding resized the page");
    // The agent shows a tab while expanded: nothing changes.
    browser.act("w1", false).unwrap();
    assert_eq!(browser.presented(), Some("expanded"));
    // The strip's collapse button goes back to the mini, the page unchanged.
    browser.click("collapse");
    until("the mini again", || browser.presented() == Some("mini"));
    assert_eq!(viewport(&driver), size, "collapsing resized the page");
    let (first, later) = frames(&driver, Duration::from_millis(500));
    assert!(
        later > first + 5,
        "collapsed, the page stopped: {first} to {later}"
    );
    browser.click_mini();
    until("expanded once more", || {
        browser.presented() == Some("expanded")
    });
    assert_eq!(viewport(&driver), size, "expanding again resized the page");
    browser.click("collapse");
    until("the mini once more", || browser.presented() == Some("mini"));

    // A screenshot is the same whichever state the window is in.
    driver.screenshot(None).expect("a picture while collapsed");

    // The last tab closed, the window and its mini are gone.
    driver.close();
    until("hidden", || browser.presented() == Some("hidden"));
}

/// Whether this process is the app the person's keys go to, polled while `during` runs.
#[cfg(target_os = "macos")]
pub(crate) fn never_active(during: impl FnOnce()) {
    use std::sync::atomic::{AtomicBool, Ordering};
    let stop = AtomicBool::new(false);
    let seen = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            while !stop.load(Ordering::SeqCst) {
                if super::panel::active() {
                    seen.store(true, Ordering::SeqCst);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        during();
        stop.store(true, Ordering::SeqCst);
    });
    assert!(
        !seen.load(Ordering::SeqCst),
        "the process became the active app: it took the person's keys"
    );
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn never_active(during: impl FnOnce()) {
    during();
}
