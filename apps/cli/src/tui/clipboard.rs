//! The person's clipboard, the interface's own (K2): `/copy`'s text is written where the person
//! is, by the system's tool, whichever server sent it.

use std::io::Write;
use std::process::{Command, Stdio};

/// Write `text` to the system clipboard; what came of it, for the person.
pub(super) fn copy(text: &str) -> String {
    #[cfg(target_os = "macos")]
    let tools: &[&[&str]] = &[&["pbcopy"]];
    #[cfg(target_os = "windows")]
    let tools: &[&[&str]] = &[&["clip"]];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let tools: &[&[&str]] = &[
        &["wl-copy"],
        &["xclip", "-selection", "clipboard"],
        &["xsel", "-ib"],
    ];
    for tool in tools {
        let spawned = Command::new(tool[0])
            .args(&tool[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut child) = spawned else {
            continue;
        };
        let written = child
            .stdin
            .take()
            .map(|mut stdin| stdin.write_all(text.as_bytes()));
        if matches!(written, Some(Ok(()))) && child.wait().is_ok_and(|s| s.success()) {
            let count = text.chars().count();
            return format!("Copied the last answer ({count} characters).\n");
        }
    }
    "No clipboard tool here (pbcopy, wl-copy, xclip, xsel or clip).\n".into()
}
