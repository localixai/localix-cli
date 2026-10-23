//! `window`: everything the agent opens to look at and work in, as tabs of one window: a page,
//! a terminal session, and kinds to come. The tool knows no kind: each one is a [`Kind`], a
//! strategy that opens its tabs, acts in them, reads them (a snapshot: text, at once after
//! opening or acting) and pictures them, and the tool gives each its ids (`w3`, `t2`) and its
//! part of the arguments. Where the window shows (`Windowed`), it lists every tab, puts one in
//! front of the person, and takes a size; showing changes nothing a tab does, so it asks no gate.
//! Each kind asks its own gate before an effect.

use contracts::gate::Gate;
use contracts::hands::Hands;
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use contracts::view::View;
use serde_json::{Map, Value, json};

#[path = "window_goal.rs"]
mod goal;
#[path = "window_tabs.rs"]
mod tabs;

const NAME: &str = "window";

const DESCRIPTION: &str = "Open things to look at and work in, as tabs of one window, and act in \
them. `open` opens one tab (`kind` and its fields) or several at once (`tabs`, a list of them) \
and returns each one's id and its snapshot: the text it shows now. `interact` acts in tab `tab` \
with that kind's fields, and returns what happened and the snapshot after. `screenshot` returns a \
picture of a tab, for what its text cannot say. `close` ends a tab (`tab`) or several (`tabs`). \
Where the person watches the window: `list` names every tab, which is shown and which the browser \
acts on; `show` puts a tab in front of them and returns its snapshot (it changes nothing in it); \
`resize` sizes the whole window (`width`, `height`). Without a window, `list` names the tabs open. \
A tab's snapshot is its newest state: read it before acting again. Calls in one turn run at \
once, each tab on its own: when several tabs each need a step, make those calls together.
- `goal` (with `interact`, any kind of tab) hands one short goal to a fast helper that sees only \
the goal and the tab, and acts in it (clicks and types on a page, types and presses keys in a \
terminal): one screen's worth of progress, ending with how to tell it is done, as something the \
tab will show (\"Answer yes to the trust question; done when the prompt shows\"). It does not \
remember earlier screens and does not read for you: read the snapshot it returns. It types only \
the exact strings in `values`. Check the snapshot yourself: its DONE is not proof.
The kinds:";

/// What can be done to a tab of the window as the person sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Show,
    Close,
}

/// A tab as the window lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowTab {
    pub id: String,
    /// What it is (`browser`, `terminal`, ...), as the window names it.
    pub kind: String,
    pub title: String,
    /// What it holds beside its title (a page's address), if anything.
    pub detail: String,
    /// It fills the window now.
    pub shown: bool,
    /// The tab the browser acts on.
    pub acted_on: bool,
}

/// The window the person watches, as whoever holds it lets it be read and changed.
pub trait Windowed: Send + Sync {
    /// The tabs, in the order the window shows them; `None` while it is not open.
    fn tabs(&self) -> Option<Vec<WindowTab>>;
    fn act(&self, act: Act, tab: &str) -> Result<(), String>;
    /// Size the whole window, in points.
    fn resize(&self, width: u32, height: u32) -> Result<(), String>;
}

/// A tab just opened: its id among its kind's, and what opening it gave.
pub struct Opened {
    pub id: u32,
    pub output: ToolOutput,
}

/// A kind of tab: how its tabs are opened, acted in, read, pictured and closed.
pub trait Kind: Send + Sync {
    /// What `kind` names in `open`, and what the window calls its tabs.
    fn name(&self) -> &'static str;
    /// The letter its tabs' ids begin with (`w3`, `t2`).
    fn letter(&self) -> char;
    /// What the model is told of it: its fields, and how to use them well.
    fn guide(&self) -> &'static str;
    /// The arguments it reads in `open` and `interact`, as the schema's properties.
    fn fields(&self) -> Vec<(&'static str, Value)>;
    fn open(&self, args: &Value) -> Result<Opened, ToolError>;
    fn interact(&self, id: u32, args: &Value) -> Result<ToolOutput, ToolError>;
    fn snapshot(&self, id: u32) -> Result<ToolOutput, ToolError>;
    fn screenshot(&self, id: u32, args: &Value) -> Result<ToolOutput, ToolError>;
    fn close(&self, id: u32) -> Result<String, ToolError>;
    /// Its tabs open now: id, title and what each holds.
    fn tabs(&self) -> Vec<(u32, String, String)>;
    /// Tab `id` as a view the helper acts on, for a `goal`.
    fn view(&self, id: u32) -> Result<Box<dyn View + '_>, ToolError>;
}

const NOT_SHOWN: &str = "The window is not shown here (no one watches it): `open`, `interact`, \
`screenshot`, `close` and `list` still work.";

pub struct WindowTool<'a> {
    pub kinds: Vec<&'a dyn Kind>,
    /// The window the person watches, where it shows.
    pub window: Option<&'a dyn Windowed>,
    /// Who carries out a `goal` on a tab's view; none where there is no decision model.
    pub hands: Option<&'a dyn Hands>,
    /// Asked before a goal is handed over: the helper acts on what the tab is connected to.
    pub gate: &'a dyn Gate,
}

impl Tool for WindowTool<'_> {
    /// A turn's window calls run at once: each kind keeps its own resources apart (a terminal
    /// session is one call's at a time, a browser tab too).
    fn parallel(&self) -> bool {
        true
    }

    fn spec(&self) -> ToolSpec {
        let guides: Vec<String> = (self.kinds.iter())
            .map(|kind| {
                format!(
                    "- {} (`{}` tabs): {}",
                    kind.name(),
                    kind.letter(),
                    kind.guide()
                )
            })
            .collect();
        let names: Vec<&str> = self.kinds.iter().map(|kind| kind.name()).collect();
        let mut properties = Map::new();
        properties.insert(
            "action".into(),
            json!({"type": "string", "enum":
            ["open", "interact", "screenshot", "close", "show", "list", "resize"]}),
        );
        properties.insert(
            "kind".into(),
            json!({"type": "string", "enum": names,
            "description": "open: the kind of tab."}),
        );
        properties.insert(
            "tabs".into(),
            json!({"type": "array", "description":
            "open: several tabs at once, each an object with `kind` and its fields; close: \
             several tab ids.", "items": {}}),
        );
        properties.insert(
            "tab".into(),
            json!({"type": "string",
            "description": "A tab's id, as `open` or `list` gave it: `w1`, `t2`."}),
        );
        properties.insert(
            "goal".into(),
            json!({"type": "string", "description":
            "interact: actions only, one screen's worth, ending with how to tell they are done. \
             Never 'find' or 'report': the helper cannot read for you."}),
        );
        properties.insert(
            "values".into(),
            json!({"type": "array", "items": {"type": "string"},
            "description": "interact: the exact strings the helper may type."}),
        );
        properties.insert("width".into(), json!({"type": "integer", "minimum": 400}));
        properties.insert("height".into(), json!({"type": "integer", "minimum": 300}));
        for kind in &self.kinds {
            for (name, field) in kind.fields() {
                properties.entry(name).or_insert(field);
            }
        }
        ToolSpec {
            name: NAME.into(),
            description: format!("{DESCRIPTION}\n{}", guides.join("\n")),
            parameters: json!({"type": "object", "required": ["action"],
                "properties": properties}),
        }
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let text = |key: &str| arguments.get(key).and_then(Value::as_str);
        match text("action") {
            Some("open") => self.open(arguments),
            Some("interact") => self.interact(arguments),
            Some("screenshot") => {
                let (_, kind, id) = self.tab(text("tab"))?;
                kind.screenshot(id, arguments)
            }
            Some("close") => self.close(arguments),
            Some("show") => {
                let window = self.window.ok_or_else(|| failed(NOT_SHOWN))?;
                let (tab, kind, id) = self.tab(text("tab"))?;
                window.act(Act::Show, &tab).map_err(ToolError::Failed)?;
                let read = kind.snapshot(id)?;
                Ok(ToolOutput {
                    report: format!("Showing {tab}."),
                    ..read
                })
            }
            Some("list") => Ok(self.list().into()),
            Some("resize") => {
                let window = self.window.ok_or_else(|| failed(NOT_SHOWN))?;
                let size = |key: &str| arguments.get(key).and_then(Value::as_u64);
                let (Some(width), Some(height)) = (size("width"), size("height")) else {
                    return Err(failed("`width` and `height` are needed, in points"));
                };
                window
                    .resize(width as u32, height as u32)
                    .map_err(ToolError::Failed)?;
                Ok(format!("The window is {width}×{height} now.").into())
            }
            _ => Err(failed(
                "`action` is one of open, interact, screenshot, close, show, list, resize",
            )),
        }
    }
}

fn failed(why: &str) -> ToolError {
    ToolError::Failed(why.into())
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
