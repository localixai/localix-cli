//! Pictures the person attaches to a message: a file dragged in or pasted as its path, the
//! clipboard's image (Ctrl+V), or `--image`. Each goes to the model after the message, as the
//! person's own; any other file is mentioned by its path (`@`), and the agent reads it.

use std::path::{Path, PathBuf};

use base64::Engine;
use contracts::tool::Image;

/// What a model takes in one picture; a larger file is refused with the reason.
const MAX_BYTES: usize = 8 * 1024 * 1024;

/// A picture attached to a message, and the name it is shown with.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Attachment {
    pub name: String,
    pub image: Image,
}

/// The picture at `path`, read and checked.
pub(crate) fn read(path: &Path) -> Result<Attachment, String> {
    let shown = path.display();
    let bytes =
        std::fs::read(path).map_err(|error| format!("{shown} could not be read: {error}"))?;
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "{shown} is {} MB; a picture may be at most 8 MB",
            bytes.len() / 1024 / 1024
        ));
    }
    let (media_type, (width, height)) =
        kind(&bytes).ok_or_else(|| format!("{shown} is not a PNG, JPEG, GIF or WebP picture"))?;
    let name = path.file_name().map_or_else(
        || shown.to_string(),
        |name| name.to_string_lossy().to_string(),
    );
    Ok(Attachment {
        name,
        image: Image {
            media_type: media_type.into(),
            data: base64::engine::general_purpose::STANDARD.encode(&bytes),
            width,
            height,
        },
    })
}

/// The picture's media type and size, from its first bytes.
pub(crate) fn kind(bytes: &[u8]) -> Option<(&'static str, (u32, u32))> {
    let be = |at: usize, n: usize| {
        bytes
            .get(at..at + n)
            .map(|b| b.iter().fold(0u32, |v, x| v << 8 | *x as u32))
    };
    let le16 = |at: usize| {
        bytes
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as u32)
    };
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(("image/png", (be(16, 4)?, be(20, 4)?)));
    }
    if bytes.starts_with(b"GIF8") {
        return Some(("image/gif", (le16(6)?, le16(8)?)));
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        let le24 = |at: usize| {
            bytes
                .get(at..at + 3)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], 0]))
        };
        let size = match bytes.get(12..16)? {
            b"VP8 " => (le16(26)? & 0x3fff, le16(28)? & 0x3fff),
            b"VP8L" => {
                let bits = u32::from_le_bytes(bytes.get(21..25)?.try_into().ok()?);
                ((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1)
            }
            b"VP8X" => (le24(24)? + 1, le24(27)? + 1),
            _ => return None,
        };
        return Some(("image/webp", size));
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        // The size is in the first start-of-frame segment.
        let mut at = 2;
        while at + 9 < bytes.len() {
            if bytes[at] != 0xff {
                return None;
            }
            let marker = bytes[at + 1];
            let length = be(at + 2, 2)? as usize;
            if (0xc0..=0xcf).contains(&marker) && ![0xc4, 0xc8, 0xcc].contains(&marker) {
                return Some(("image/jpeg", (be(at + 7, 2)?, be(at + 5, 2)?)));
            }
            at += 2 + length;
        }
    }
    None
}

/// A file named in what the person typed or dropped in.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Dropped {
    /// A picture: sent to the model as one.
    Image(PathBuf),
    /// Any other file or folder: named by its path, for the agent to read.
    File(PathBuf),
}

/// What `token` names, when it is a path that exists: as a terminal writes a dropped file
/// (quoted, its spaces escaped, or as a `file://` address) or as typed (`~/x`).
pub(crate) fn resolve(token: &str) -> Option<Dropped> {
    let token = token.trim();
    let unquoted = token
        .strip_prefix('\'')
        .and_then(|t| t.strip_suffix('\''))
        .or_else(|| token.strip_prefix('"').and_then(|t| t.strip_suffix('"')));
    let path = match unquoted {
        Some(path) => path.to_string(),
        None => unescape(token),
    };
    let path = path
        .strip_prefix("file://")
        .map_or(path.clone(), |p| p.replace("%20", " "));
    let path = match path.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var_os("HOME")?).join(rest),
        None => PathBuf::from(&path),
    };
    if !path.is_absolute() || !path.exists() {
        return None;
    }
    let lower = path.to_string_lossy().to_ascii_lowercase();
    let picture = [".png", ".jpg", ".jpeg", ".gif", ".webp"]
        .iter()
        .any(|ext| lower.ends_with(ext));
    Some(if picture && path.is_file() {
        Dropped::Image(path)
    } else {
        Dropped::File(path)
    })
}

/// A shell word's backslash escapes undone: `a\ b` is `a b`.
fn unescape(word: &str) -> String {
    let mut out = String::new();
    let mut chars = word.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.extend(chars.next()),
            c => out.push(c),
        }
    }
    out
}

/// A copy of the picture at `path` under `dir`, so a file the system removes soon (a screenshot's
/// temporary file) is still there when the message is sent; the original when it cannot be made.
pub(crate) fn keep(path: &Path, dir: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let copy = dir.join(format!("{stamp}-{name}"));
    match std::fs::create_dir_all(dir).and_then(|()| std::fs::copy(path, &copy)) {
        Ok(_) => copy,
        Err(_) => path.to_path_buf(),
    }
}

/// Where pictures the person pasted or dropped in are kept: beside the sessions, so they outlive
/// the files they came from.
pub(crate) fn pasted_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".llx").join("pasted")
}

/// The clipboard's picture, saved as a PNG under `dir`; why not when there is none.
#[cfg(target_os = "macos")]
pub(crate) fn clipboard(dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis());
    let path = dir.join(format!("pasted-{stamp}.png"));
    // AppleScript writes the clipboard's PNG data to the file; it fails when there is none.
    let script = format!(
        "set f to open for access POSIX file \"{}\" with write permission\n\
         write (the clipboard as «class PNGf») to f\nclose access f",
        path.display()
    );
    let ran = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output();
    match ran {
        Ok(out) if out.status.success() && path.metadata().is_ok_and(|m| m.len() > 0) => Ok(path),
        _ => {
            let _ = std::fs::remove_file(&path);
            Err("the clipboard holds no picture".into())
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn clipboard(_: &Path) -> Result<PathBuf, String> {
    Err("pasting a picture is not supported here yet: drag the file in instead".into())
}

#[cfg(test)]
#[path = "attach_tests.rs"]
mod tests;
