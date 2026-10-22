//! The tools the decision loop can drive, one module each. Every tool implements the tool protocol
//! (`agent::tool::Tool`) and knows nothing of the loop or the model.
//! - [`browser`]: a native browser window on the system WebView.

pub mod browser;
