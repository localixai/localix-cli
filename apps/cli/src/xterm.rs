//! xterm.js, which draws the terminal sessions' windows. It is not in the binary: a session that
//! shows windows fetches the pinned files once from jsDelivr, in the background, checks each
//! against the SHA-384 pinned here, and keeps them in `~/.llx/cache/xterm/`. A window opens on
//! what is kept, checked again; with nothing kept (offline, or a file that does not match), it
//! shows the plain grid instead. `exec` never fetches.

use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use sha2::{Digest, Sha384};
use tools::shell::{Asset, Emulator};

/// A file as it is fetched and checked.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Pin {
    pub name: &'static str,
    pub url: &'static str,
    /// Base64, as a page's policy names a script.
    pub sha384: &'static str,
}

/// The emulator, its fit addon, and its stylesheet, at exact versions.
pub(crate) const PINS: [Pin; 3] = [
    Pin {
        name: "xterm.js",
        url: "https://cdn.jsdelivr.net/npm/@xterm/xterm@6.0.0/lib/xterm.js",
        sha384: "f/1U6Z9wM4D71a5eRXEZnyOTMOvjqxr2XLwh+Go1OvIl3L3tOcvUrzudnhbECwl4",
    },
    Pin {
        name: "addon-fit.js",
        url: "https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0.11.0/lib/addon-fit.js",
        sha384: "txoiwu4RR2GD3qySbaj+BbzibkLbSJRcfqGYMu6z1EqHil4A2dyBiBW5dlacG6OR",
    },
    Pin {
        name: "xterm.css",
        url: "https://cdn.jsdelivr.net/npm/@xterm/xterm@6.0.0/css/xterm.css",
        sha384: "n2n7twoohnW+d3myBKaUgl7DSiwidw6MkQy9oesGzkPpMjejKRR3XlnD+5yCdtBD",
    },
];

/// The longest file taken: xterm.js is under half a megabyte.
const LIMIT: u64 = 4 << 20;
const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn sha384(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(Sha384::digest(bytes))
}

/// Where the files are kept: `~/.llx/cache/xterm`.
pub(crate) fn dir(home: Option<PathBuf>) -> Option<PathBuf> {
    Some(home?.join(".llx").join("cache").join("xterm"))
}

/// The kept file of `pin`, when it is the one pinned.
fn kept(dir: &Path, pin: &Pin) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(pin.name)).ok()?;
    (sha384(text.as_bytes()) == pin.sha384).then_some(text)
}

/// The emulator as kept in `dir`, each file checked against its pin; none unless all are good.
pub(crate) fn load(dir: &Path, [script, fit, css]: &[Pin; 3]) -> Option<Emulator> {
    let asset = |pin: &Pin| {
        let text = kept(dir, pin)?;
        let sha384 = pin.sha384.to_string();
        Some(Asset { text, sha384 })
    };
    Some(Emulator {
        script: asset(script)?,
        fit: asset(fit)?,
        css: kept(dir, css)?,
    })
}

/// Fetch with `get` the files of `pins` that `dir` does not hold as pinned, and keep each that
/// matches its pin, whole or not at all; how many were fetched.
pub(crate) fn fetch(
    dir: &Path,
    pins: &[Pin; 3],
    get: &dyn Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<usize, String> {
    let mut fetched = 0;
    for pin in pins.iter().filter(|pin| kept(dir, pin).is_none()) {
        let bytes = get(pin.url)?;
        if sha384(&bytes) != pin.sha384 {
            return Err(format!("{} does not match its pinned SHA-384", pin.name));
        }
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        let path = dir.join(pin.name);
        let temporary = path.with_extension(format!("tmp{}", std::process::id()));
        let written =
            std::fs::write(&temporary, &bytes).and_then(|_| std::fs::rename(&temporary, &path));
        if let Err(error) = written {
            let _ = std::fs::remove_file(&temporary);
            return Err(error.to_string());
        }
        fetched += 1;
    }
    Ok(fetched)
}

/// Whether to fetch: in a session whose terminal sessions show in windows, unless `LLX_XTERM=0`
/// (as `var` reads the environment).
pub(crate) fn wanted(session: bool, windowed: bool, var: impl Fn(&str) -> Option<String>) -> bool {
    session && windowed && var("LLX_XTERM").as_deref() != Some("0")
}

/// The emulator kept for this person, if any.
pub(crate) fn installed() -> Option<Emulator> {
    load(&dir(std::env::var_os("HOME").map(PathBuf::from))?, &PINS)
}

/// Fetch what is missing in the background; nothing waits on it, and a failure changes nothing.
pub(crate) fn prefetch() {
    let Some(dir) = dir(std::env::var_os("HOME").map(PathBuf::from)) else {
        return;
    };
    let fetching = std::thread::Builder::new().name("llx-xterm".into());
    // No thread: nothing is fetched, as when the network fails.
    let _ = fetching.spawn(move || {
        let _ = fetch(&dir, &PINS, &web);
    });
}

/// One file over HTTPS, with the agent the providers use.
fn web(url: &str) -> Result<Vec<u8>, String> {
    let agent = provider_kit::http::agent(crate::compose::APP, TIMEOUT);
    let mut response = agent.get(url).call().map_err(|error| error.to_string())?;
    if response.status() != 200 {
        return Err(format!("{url} answered {}", response.status()));
    }
    let body = response.body_mut().with_config().limit(LIMIT);
    body.read_to_vec().map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "xterm_tests.rs"]
mod tests;
