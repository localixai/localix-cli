//! The person's clipboard, the interface's own (K2): `/copy`'s text is written where the person
//! is, by the system's tool, whichever server sent it.

use std::process::Command;
use std::time::Duration;

use super::within::within;

/// How long the system's tool may take to take the text (R10).
const WAIT: Duration = Duration::from_secs(3);

/// Write `text` to the system clipboard; what came of it, for the person.
pub(super) fn copy(text: &str) -> String {
    #[cfg(target_os = "macos")]
    let tools: &[&[&str]] = &[&["pbcopy"]];
    #[cfg(not(target_os = "macos"))]
    let tools: &[&[&str]] = &[
        &["wl-copy"],
        &["xclip", "-selection", "clipboard"],
        &["xsel", "-ib"],
    ];
    for tool in tools {
        let mut command = Command::new(tool[0]);
        command.args(&tool[1..]);
        if within(command, text.as_bytes().to_vec(), WAIT).is_some() {
            let count = text.chars().count();
            return format!("Copied the last answer ({count} characters).\n");
        }
    }
    "No clipboard tool here (pbcopy, wl-copy, xclip, xsel or clip).\n".into()
}
