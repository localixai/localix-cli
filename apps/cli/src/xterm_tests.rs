use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::testing::Scratch;

/// Pins for three small files, as the fake network serves them.
fn pins() -> [Pin; 3] {
    let pin = |name: &'static str, text: &str| Pin {
        name,
        url: Box::leak(format!("https://cdn.test/{name}").into_boxed_str()),
        sha384: Box::leak(sha384(text.as_bytes()).into_boxed_str()),
    };
    [
        pin("xterm.js", "var Terminal;"),
        pin("addon-fit.js", "var FitAddon;"),
        pin("xterm.css", ".xterm{}"),
    ]
}

/// The network as a test serves it: each file's text, `tampered` changed in transit.
fn network(tampered: Option<&'static str>) -> impl Fn(&str) -> Result<Vec<u8>, String> {
    move |url: &str| {
        let name = url.rsplit('/').next().unwrap();
        let text = match name {
            "xterm.js" => "var Terminal;",
            "addon-fit.js" => "var FitAddon;",
            _ => ".xterm{}",
        };
        let text = match tampered == Some(name) {
            true => format!("{text} alert(1)"),
            false => text.to_string(),
        };
        Ok(text.into_bytes())
    }
}

fn files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            let names = entries.flatten();
            names
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn a_good_download_is_kept_and_then_shown() {
    let (scratch, pins) = (Scratch::new(), pins());
    let dir = scratch.path().join("cache/xterm");
    assert_eq!(fetch(&dir, &pins, &network(None)), Ok(3));
    assert_eq!(files(&dir), ["addon-fit.js", "xterm.css", "xterm.js"]);
    let emulator = load(&dir, &pins).expect("the emulator, verified");
    assert_eq!(emulator.script.text, "var Terminal;");
    assert_eq!(emulator.script.sha384, pins[0].sha384);
    assert_eq!(emulator.fit.text, "var FitAddon;");
    assert_eq!(emulator.css, ".xterm{}");
}

#[test]
fn a_download_that_does_not_match_its_pin_is_never_kept() {
    let (scratch, pins) = (Scratch::new(), pins());
    let dir = scratch.path().join("xterm");
    let fetched = fetch(&dir, &pins, &network(Some("addon-fit.js")));
    assert!(
        matches!(&fetched, Err(why) if why.contains("addon-fit.js")),
        "{fetched:?}"
    );
    assert!(
        !files(&dir).contains(&"addon-fit.js".to_string()),
        "{:?}",
        files(&dir)
    );
    assert!(load(&dir, &pins).is_none(), "the grid shows instead");
}

#[test]
fn without_a_network_nothing_is_kept_and_the_grid_shows() {
    let (scratch, pins) = (Scratch::new(), pins());
    let dir = scratch.path().join("xterm");
    let offline = |_: &str| Err::<Vec<u8>, _>("dns error".to_string());
    assert!(fetch(&dir, &pins, &offline).is_err());
    assert!(files(&dir).is_empty(), "{:?}", files(&dir));
    assert!(load(&dir, &pins).is_none());
}

#[test]
fn what_is_kept_is_not_fetched_again_and_a_changed_file_is() {
    let (scratch, pins) = (Scratch::new(), pins());
    let dir = scratch.path().join("xterm");
    fetch(&dir, &pins, &network(None)).unwrap();
    let asked = AtomicUsize::new(0);
    let counted = |url: &str| {
        asked.fetch_add(1, Ordering::SeqCst);
        network(None)(url)
    };
    assert_eq!(fetch(&dir, &pins, &counted), Ok(0));
    assert_eq!(
        asked.load(Ordering::SeqCst),
        0,
        "already cached: no network"
    );
    // A kept file changed on disk is not run, and it alone is fetched again.
    std::fs::write(dir.join("xterm.js"), "var Terminal; steal()").unwrap();
    assert!(load(&dir, &pins).is_none());
    assert_eq!(fetch(&dir, &pins, &counted), Ok(1));
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    assert!(load(&dir, &pins).is_some());
}

#[test]
fn the_pins_are_exact_versions_on_jsdelivr() {
    for pin in &PINS {
        assert!(
            pin.url.starts_with("https://cdn.jsdelivr.net/npm/@xterm/"),
            "{}",
            pin.url
        );
        assert!(pin.url.contains("@6.0.0/") || pin.url.contains("@0.11.0/"));
        assert!(pin.url.ends_with(pin.name));
        assert_eq!(pin.sha384.len(), 64, "a SHA-384 in base64");
    }
    assert_eq!(sha384(b"abc").len(), 64);
}

#[test]
fn it_is_fetched_for_a_session_with_windows_only() {
    let unset = |_: &str| None;
    assert!(wanted(true, true, unset));
    assert!(!wanted(false, true, unset), "exec never fetches");
    assert!(!wanted(true, false, unset), "no windows, no fetch");
    let off = |name: &str| (name == "LLX_XTERM").then(|| "0".to_string());
    assert!(!wanted(true, true, off));
}

#[test]
fn the_cache_is_in_the_persons_llx_folder() {
    let home = PathBuf::from("/home/someone");
    assert_eq!(
        dir(Some(home)),
        Some(PathBuf::from("/home/someone/.llx/cache/xterm"))
    );
    assert_eq!(dir(None), None);
}
