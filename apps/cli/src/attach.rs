//! Pictures attached to a message, read for the model: what the interface sent as paths (a file
//! dragged in, the clipboard's image: `tui::attach`) or `--image`. Each goes to the model after
//! the message, as the person's own.

use std::path::Path;

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

#[cfg(test)]
#[path = "attach_tests.rs"]
mod tests;
