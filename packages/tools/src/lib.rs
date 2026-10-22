//! The tools, one module each. A tool implements the agent's tool protocol
//! (`contracts::tool::Tool`) and, for what the hands drive, the view protocol
//! (`contracts::view::View`); it knows nothing of the agent, the hands or the models.
//! - [`browser`]: a native browser window on the system WebView.

pub mod browser;
