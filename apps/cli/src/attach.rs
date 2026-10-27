//! Pictures attached to a message, read for the model: what the interface sent as paths (a file
//! dragged in, the clipboard's image: `tui::attach`) or `--image`. Each goes to the model after
//! the message, as the person's own.

use std::path::Path;

use contracts::tool::Image;
use tools::fs::picture::{self, MAX_BYTES, MAX_SHOWN};

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
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_BYTES {
        return Err(format!(
            "{shown} is {} MB; a picture may be at most {MAX_SHOWN}",
            bytes.len() / 1024 / 1024
        ));
    }
    let image = picture::image(&bytes)
        .ok_or_else(|| format!("{shown} is not a PNG, JPEG, GIF or WebP picture"))?;
    let name = path.file_name().map_or_else(
        || shown.to_string(),
        |name| name.to_string_lossy().to_string(),
    );
    Ok(Attachment { name, image })
}

#[cfg(test)]
#[path = "attach_tests.rs"]
mod tests;
