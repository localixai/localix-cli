//! The agent's tool protocol: a tool says what it is for and what arguments it takes, and turns a
//! call into text for the model. A tool knows nothing of the agent or the model that calls it.

use serde_json::Value;

/// How a tool is offered to the model.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSpec {
    pub name: String,
    /// What it does and how to use it well: all the model knows of the tool's world.
    pub description: String,
    /// The arguments, as a JSON Schema object.
    pub parameters: Value,
}

/// What a call did, as the model reads it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolOutput {
    /// What happened: kept for the rest of the conversation.
    pub report: String,
    /// The state the call left (a view, a screen): only the newest one matters, so an agent may
    /// drop older ones to keep the conversation short.
    pub state: Option<String>,
    /// A picture of what the call left (a screenshot), for a model that reads images: like a
    /// state, only the newest few matter.
    pub image: Option<Image>,
}

/// A report alone: most calls leave no state and no picture.
impl From<String> for ToolOutput {
    fn from(report: String) -> Self {
        Self {
            report,
            ..Self::default()
        }
    }
}

/// An encoded picture, as a model reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    /// `image/jpeg`, `image/png`.
    pub media_type: String,
    /// The encoded bytes, in base64.
    pub data: String,
    /// In pixels.
    pub width: u32,
    pub height: u32,
}

/// Why a call did nothing useful.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolError {
    /// The call failed, but another call can do better (wrong arguments, a view that refused): the
    /// model reads why.
    Failed(String),
    /// Nothing will work now (what the tool depends on is unreachable): the run ends.
    Fatal(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed(message) | Self::Fatal(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ToolError {}

pub trait Tool: Send + Sync {
    fn spec(&self) -> ToolSpec;
    /// Do what `arguments` ask and say what happened.
    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError>;
    /// Whether its calls may run at the same time as other such calls of one turn: they change
    /// nothing another call of the turn reads (reading, searching, commands). A call that is not
    /// runs alone, in its turn's order.
    fn parallel(&self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
