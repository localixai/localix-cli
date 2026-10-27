//! Image file strategy: recognize signatures, bound reads and return encoded pictures.

use std::io::Read;

use base64::Engine as _;
use contracts::tool::{Image, ToolError, ToolOutput};

use super::{Args, failure};

const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) enum Format {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl Format {
    pub(super) fn detect(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.starts_with(b"\xff\xd8\xff") {
            Some(Self::Jpeg)
        } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            Some(Self::Gif)
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            Some(Self::Webp)
        } else {
            None
        }
    }

    fn media_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }

    fn dimensions(self, bytes: &[u8]) -> Option<(u32, u32)> {
        match self {
            Self::Png if bytes.get(12..16)? == b"IHDR" => {
                Some((be32(bytes.get(16..20)?)?, be32(bytes.get(20..24)?)?))
            }
            Self::Gif => Some((le16(bytes.get(6..8)?)?, le16(bytes.get(8..10)?)?)),
            Self::Jpeg => jpeg_dimensions(bytes),
            Self::Webp => webp_dimensions(bytes),
            Self::Png => None,
        }
        .filter(|&(width, height)| width > 0 && height > 0)
    }
}

pub(super) fn read(
    reader: impl Read,
    shown: &str,
    size: u64,
    format: Format,
    args: &Args,
) -> Result<ToolOutput, ToolError> {
    if args.offset.is_some() || args.limit.is_some() {
        return Err(ToolError::Failed(format!(
            "{shown} is an image: offset and limit apply only to text"
        )));
    }
    if size > MAX_IMAGE_BYTES {
        return Err(too_large(shown));
    }
    let mut bytes = Vec::new();
    reader
        .take(MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| failure(shown, &error))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_IMAGE_BYTES {
        return Err(too_large(shown));
    }
    let (width, height) = format.dimensions(&bytes).ok_or_else(|| {
        ToolError::Failed(format!("{shown} has an invalid or incomplete image header"))
    })?;
    Ok(ToolOutput {
        report: format!("{shown} ({}, {width} × {height})", format.media_type()),
        image: Some(Image {
            media_type: format.media_type().into(),
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
            width,
            height,
        }),
        state: None,
    })
}

fn too_large(shown: &str) -> ToolError {
    ToolError::Failed(format!("{shown} is larger than the 5 MiB image limit"))
}

fn le16(bytes: &[u8]) -> Option<u32> {
    Some(u32::from(u16::from_le_bytes(bytes.try_into().ok()?)))
}

fn be32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.try_into().ok()?))
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut pos = 2;
    while pos < bytes.len() {
        if *bytes.get(pos)? != 0xff {
            return None;
        }
        while *bytes.get(pos)? == 0xff {
            pos += 1;
        }
        let marker = *bytes.get(pos)?;
        pos += 1;
        if marker == 0xd9 || marker == 0xda {
            return None;
        }
        if marker == 0x01 || (0xd0..=0xd8).contains(&marker) {
            continue;
        }
        let length = usize::from(u16::from_be_bytes(
            bytes.get(pos..pos + 2)?.try_into().ok()?,
        ));
        if length < 2 || pos.checked_add(length)? > bytes.len() {
            return None;
        }
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            if length < 7 {
                return None;
            }
            return Some((
                u32::from(u16::from_be_bytes(
                    bytes.get(pos + 5..pos + 7)?.try_into().ok()?,
                )),
                u32::from(u16::from_be_bytes(
                    bytes.get(pos + 3..pos + 5)?.try_into().ok()?,
                )),
            ));
        }
        pos += length;
    }
    None
}

fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    match bytes.get(12..16)? {
        b"VP8X" => {
            let b = bytes.get(24..30)?;
            Some((
                1 + u32::from(b[0]) + (u32::from(b[1]) << 8) + (u32::from(b[2]) << 16),
                1 + u32::from(b[3]) + (u32::from(b[4]) << 8) + (u32::from(b[5]) << 16),
            ))
        }
        b"VP8L" if *bytes.get(20)? == 0x2f => {
            let b = bytes.get(21..25)?;
            let bits = u32::from_le_bytes(b.try_into().ok()?);
            Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
        }
        b"VP8 " if bytes.get(23..26)? == b"\x9d\x01\x2a" => Some((
            le16(bytes.get(26..28)?)? & 0x3fff,
            le16(bytes.get(28..30)?)? & 0x3fff,
        )),
        _ => None,
    }
}
