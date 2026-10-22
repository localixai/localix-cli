//! What may load in the window, and the per-run token that keeps a page from answering for the probe.

/// Only web pages load in the window; other schemes (files, apps, mail) are refused. `data:` is
/// out too: WebKit drops IPC from its opaque origin, so the probe could never answer there.
pub(super) fn allowed(url: &str) -> bool {
    ["http://", "https://", "about:", "blob:"]
        .iter()
        .any(|scheme| url.starts_with(scheme))
}

/// 32 hex characters from the clock and the address space; unguessable to a page in one run.
pub(super) fn token() -> String {
    use std::collections::hash_map::RandomState;
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
        out.push_str(&format!("{:016x}", hasher.finish()));
    }
    out
}
