//! `read`'s strategy for a picture (`super::picture`): the whole file, bounded, sent to the model
//! as one.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};

use contracts::tool::{ToolError, ToolOutput};

use super::{Args, FileStrategy, failure};
use crate::fs::picture::{self, MAX_BYTES, MAX_SHOWN};

pub(super) struct Images;

impl FileStrategy for Images {
    fn read(
        &self,
        reader: &mut BufReader<File>,
        shown: &str,
        size: u64,
        args: &Args,
    ) -> Result<Option<ToolOutput>, ToolError> {
        let header = reader.fill_buf().map_err(|error| failure(shown, &error))?;
        if !picture::begins(header) {
            return Ok(None);
        }
        read(reader, shown, size, args).map(Some)
    }
}

/// The picture `reader` holds; `size` is what the file's metadata said, and the read is bounded
/// all the same: a file may grow after it was looked at.
pub(super) fn read(
    reader: impl Read,
    shown: &str,
    size: u64,
    args: &Args,
) -> Result<ToolOutput, ToolError> {
    if args.offset.is_some() || args.limit.is_some() {
        return Err(ToolError::Failed(format!(
            "{shown} is a picture: offset and limit apply only to text"
        )));
    }
    let too_large = || {
        ToolError::Failed(format!(
            "{shown} is larger than {MAX_SHOWN}, the most a picture may be"
        ))
    };
    if size > MAX_BYTES {
        return Err(too_large());
    }
    let mut bytes = Vec::new();
    reader
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| failure(shown, &error))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_BYTES {
        return Err(too_large());
    }
    let image = picture::image(&bytes).ok_or_else(|| {
        ToolError::Failed(format!(
            "{shown} has an invalid or incomplete picture header"
        ))
    })?;
    Ok(ToolOutput {
        report: format!(
            "{shown} ({}, {} × {})",
            image.media_type, image.width, image.height
        ),
        image: Some(image),
        state: None,
    })
}
