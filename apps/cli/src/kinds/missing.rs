//! Browser tabs where there is no browser (no display, or a build without one, ARCHITECTURE.md
//! A8): the kind is still listed, so the model reads that pages cannot open here, and `open` fails
//! with the reason instead of "no such kind". Terminal tabs are not affected.

use contracts::tool::{ToolError, ToolOutput};
use contracts::view::View;
use serde_json::Value;
use tools::window::{Kind, Opened};

pub(crate) struct Missing;

const GUIDE: &str = "Not available here: `open` fails and says why. Read a page's text with a \
command in `bash`, or work in a terminal tab.";

/// Why nothing of this kind works, as the call reports it.
fn why() -> ToolError {
    ToolError::Failed(format!(
        "pages cannot be opened here: {}",
        tools::browser::NO_WINDOW
    ))
}

impl Kind for Missing {
    fn name(&self) -> &'static str {
        "browser"
    }

    fn letter(&self) -> char {
        'w'
    }

    fn guide(&self) -> &'static str {
        GUIDE
    }

    fn fields(&self) -> Vec<(&'static str, Value)> {
        vec![]
    }

    fn open(&self, _: &Value) -> Result<Opened, ToolError> {
        Err(why())
    }

    fn interact(&self, _: u32, _: &Value) -> Result<ToolOutput, ToolError> {
        Err(why())
    }

    fn snapshot(&self, _: u32) -> Result<ToolOutput, ToolError> {
        Err(why())
    }

    fn screenshot(&self, _: u32, _: &Value) -> Result<ToolOutput, ToolError> {
        Err(why())
    }

    fn close(&self, _: u32) -> Result<String, ToolError> {
        Err(why())
    }

    fn tabs(&self) -> Vec<(u32, String, String)> {
        vec![]
    }

    fn view(&self, _: u32) -> Result<Box<dyn View + '_>, ToolError> {
        Err(why())
    }
}
