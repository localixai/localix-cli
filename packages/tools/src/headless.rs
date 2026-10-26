//! The browser of the headless build (ARCHITECTURE.md A8): `tools::browser` with nothing behind
//! its names, so the app compiles the same code without the `browser` feature and links no engine
//! (no wry, tao or WebKitGTK), for CI and servers where none is installed.
//!
//! Every handle here has a private field, so only this crate could make one, and nothing does:
//! `has_display` is false, so the app never calls `host`, never gets a `Browser`, hence never a
//! `Page`, and no tab of a page opens. Were one made, it answers as a window that is gone. The
//! terminal sessions' tabs do not depend on any of it (they are `shell`'s).

use contracts::gate::Gate;
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use contracts::view::{Action, Observation, View, ViewError, ViewResult};
use serde_json::{Value, json};

#[path = "browser/told.rs"]
mod told;

pub use told::{Heard, Listed, MAX_TABS, Tab};

/// What the model is told of browser tabs: here, that there are none.
pub const GUIDE: &str = "Not available in this build: it has no browser.";

/// Why there is no window, as the person and the model read it.
pub const NO_WINDOW: &str = "this build has no browser";

/// What would give it one: nothing short of another build.
pub const NO_WINDOW_HINT: &str = "";

/// No window can be shown here.
pub fn has_display() -> bool {
    false
}

/// An address cannot open: there is no browser to open it in.
pub fn address(_url: &str) -> Result<String, ToolError> {
    Err(ToolError::Failed(NO_WINDOW.into()))
}

/// The browser's host, without a window: `work` runs at once, on this thread, with a handle that
/// opens nothing. The app calls it only where `has_display` is true, which it never is here.
pub fn host<F>(_on_close: fn(), work: F) -> i32
where
    F: FnOnce(Browser) -> i32 + Send + 'static,
{
    work(Browser(()))
}

fn gone<T>() -> Result<T, String> {
    Err(NO_WINDOW.into())
}

fn unavailable<T>() -> ViewResult<T> {
    Err(ViewError::Unavailable(NO_WINDOW.into()))
}

/// The browser's handle: a window that is not there.
#[derive(Clone)]
pub struct Browser(());

impl Browser {
    pub fn listen(&self, _listener: impl Fn(&str, Heard<'_>) + Send + Sync + 'static) {}

    pub fn listing(&self) -> Option<Vec<Listed>> {
        None
    }

    pub fn resize_window(&self, _width: f64, _height: f64) -> Result<(), String> {
        gone()
    }

    pub fn act(&self, _id: &str, _close: bool) -> Result<(), String> {
        gone()
    }

    pub fn group(&self, _key: &str, _group: &str) {}

    pub fn panel(&self, _id: u32, _title: &str, _html: String, _ground: Option<[u8; 3]>) -> Panel {
        Panel(())
    }
}

/// A tab that shows a caller's page: painting it says the window is gone.
pub struct Panel(());

impl Panel {
    pub fn id(&self) -> u32 {
        0
    }

    pub fn paint(&self, _frame: String) -> bool {
        false
    }
}

/// The window as a view: never open.
pub struct Page(());

impl Page {
    pub fn new(_browser: Browser, _headless: bool) -> Self {
        Self(())
    }

    pub fn is_open(&self) -> bool {
        false
    }

    pub fn driver(&self) -> ViewResult<WebviewDriver> {
        unavailable()
    }

    pub fn pinned(&self, _id: u32) -> Self {
        Self(())
    }
}

impl View for Page {
    fn conventions(&self) -> &str {
        ""
    }

    fn observe(&self) -> ViewResult<Observation> {
        unavailable()
    }

    fn act(&self, _epoch: &str, _action: &Action) -> ViewResult<()> {
        unavailable()
    }
}

/// The browser's own driver: it has no tabs and opens none.
#[derive(Clone)]
pub struct WebviewDriver(());

impl WebviewDriver {
    pub fn tabs(&self) -> (Vec<Tab>, u32) {
        (vec![], 0)
    }

    pub fn add_tab(&self, _url: &str) -> ViewResult<u32> {
        unavailable()
    }

    pub fn wait_new(&self) {}
}

/// The agent's `browser`, offered alone: it says there is none.
pub struct BrowserTool<'a> {
    pub page: &'a Page,
    pub gate: &'a dyn Gate,
}

impl Tool for BrowserTool<'_> {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "browser".into(),
            description: GUIDE.into(),
            parameters: json!({"type": "object", "properties": {}}),
        }
    }

    fn call(&self, _arguments: &Value) -> Result<ToolOutput, ToolError> {
        Err(ToolError::Failed(NO_WINDOW.into()))
    }
}
