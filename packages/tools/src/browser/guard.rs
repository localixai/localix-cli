//! What may load in the window, and the per-run token that keeps a page from answering for the probe.

/// Only web pages load in the window; other schemes (files, apps, mail) are refused. `data:` is
/// out too: WebKit drops IPC from its opaque origin, so the probe could never answer there.
pub(super) fn allowed(url: &str) -> bool {
    ["http://", "https://", "about:", "blob:"]
        .iter()
        .any(|scheme| url.starts_with(scheme))
}

/// `url` as the window can load it: an address the system's URL type takes, spelled so (a space
/// or a letter past ASCII encoded); `None` for what is not an address at all. WebKit's load of
/// an address the system cannot read panics on the main thread, ending the whole process, so
/// nothing reaches a load without passing here.
pub(super) fn loadable(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    let web = matches!(parsed.scheme(), "http" | "https");
    if web && parsed.host_str().is_none_or(str::is_empty) {
        return None;
    }
    // What the person or the model wrote stays as written when it needs no spelling out.
    let plain = url.chars().all(|c| c.is_ascii_graphic());
    Some(match plain {
        true => url.to_string(),
        false => parsed.to_string(),
    })
}

/// 32 hex characters from the clock and the address space; unguessable to a page in one run.
pub(super) fn token() -> String {
    use std::collections::hash_map::RandomState;
    use std::fmt::Write as _;
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for _ in 0..2 {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        );
        let _ = write!(out, "{:016x}", hasher.finish());
    }
    out
}
