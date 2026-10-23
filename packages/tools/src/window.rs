//! `window`: everything the agent opens to look at and work in, as tabs of one window: a page,
//! a terminal session, and kinds to come. The tool knows no kind: each one is a [`Kind`], a
//! strategy that opens its tabs, acts in them, reads them (a snapshot: text, at once after
//! opening or acting) and pictures them, and the tool gives each its ids (`w3`, `t2`) and its
//! part of the arguments. Where the window shows (`Windowed`), it lists every tab, puts one in
//! front of the person, and takes a size; showing changes nothing a tab does, so it asks no gate.
//! Each kind asks its own gate before an effect.

use contracts::gate::Gate;
use contracts::hands::{Hands, Outcome, Report, Task};
use contracts::tool::{Tool, ToolError, ToolOutput, ToolSpec};
use contracts::view::View;
use serde_json::{Map, Value, json};

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

impl WindowTool<'_> {
    /// A goal for the helper on the tab's view first, then the kind's own fields; the snapshot
    /// after.
    fn interact(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let (tab, kind, id) = self.tab(arguments.get("tab").and_then(Value::as_str))?;
        let goal = (arguments.get("goal").and_then(Value::as_str))
            .map(str::trim)
            .filter(|goal| !goal.is_empty());
        let Some(goal) = goal else {
            let done = kind.interact(id, arguments)?;
            return Ok(ToolOutput {
                report: format!("{tab}: {}", done.report),
                ..done
            });
        };
        let values: Vec<String> = (arguments.get("values").and_then(Value::as_array))
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        let hands = self.hands.ok_or_else(|| {
            failed("there is no helper here (no decision model): act with the tab's own fields")
        })?;
        // The kind's own fields first (an address to load), so the goal acts on what they leave.
        let own = kind
            .fields()
            .iter()
            .any(|(name, _)| arguments.get(*name).is_some());
        let first = match own {
            true => Some(kind.interact(id, arguments)?),
            false => None,
        };
        let mut summary = format!("goal on {tab}: {}", goal.replace('\n', " \u{23ce} "));
        if !values.is_empty() {
            let quoted: Vec<String> = values.iter().map(|v| format!("\"{v}\"")).collect();
            summary.push_str(&format!(" (may type {})", quoted.join(", ")));
        }
        crate::ask(self.gate, kind.name(), &summary)?;
        let done = {
            let view = kind.view(id)?;
            hands.carry_out(
                view.as_ref(),
                Task {
                    goal,
                    values: &values,
                },
            )
        };
        if let Outcome::Unreachable(why) = &done.outcome {
            return Err(ToolError::Fatal(why.clone()));
        }
        let mut report = vec![];
        if let Some(first) = first.filter(|first| !first.report.is_empty()) {
            report.push(format!("{tab}: {}", first.report));
        }
        report.push(format!("{tab}: {}", helper(&done, &values)));
        // What the tab says after, as an action leaves it (a page's changes, a terminal's new
        // output).
        let after = kind.interact(id, &json!({}))?;
        if !after.report.is_empty() {
            report.push(after.report);
        }
        Ok(ToolOutput {
            report: report.join("\n\n"),
            ..after
        })
    }

    /// Open each tab asked for; one that fails says why, and the others still open.
    fn open(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let asked: Vec<&Value> = match arguments.get("tabs").and_then(Value::as_array) {
            Some(tabs) => tabs.iter().collect(),
            None => vec![arguments],
        };
        let mut kinds = vec![];
        for spec in &asked {
            let kind = spec.get("kind").and_then(Value::as_str);
            let Some(kind) = self.kinds.iter().find(|k| Some(k.name()) == kind) else {
                return Err(failed(&format!("`kind` is one of: {}", self.names())));
            };
            kinds.push(*kind);
        }
        // Every tab of the call opens at once; the reports keep the order they were asked in.
        let opened: Vec<Result<Opened, ToolError>> = std::thread::scope(|scope| {
            let opening: Vec<_> = (kinds.iter().zip(&asked))
                .map(|(kind, spec)| scope.spawn(move || kind.open(spec)))
                .collect();
            let joined = opening.into_iter().map(|thread| thread.join());
            joined
                .map(|done| done.unwrap_or_else(|_| Err(failed("opening it failed"))))
                .collect()
        });
        let (mut reports, mut states, mut image) = (vec![], vec![], None);
        for (kind, opened) in kinds.into_iter().zip(opened) {
            match opened {
                Ok(opened) => {
                    let tab = format!("{}{}", kind.letter(), opened.id);
                    reports.push(format!("{tab} ({}): {}", kind.name(), opened.output.report));
                    if let Some(state) = opened.output.state {
                        states.push(format!("### {tab}\n{state}"));
                    }
                    image = opened.output.image.or(image);
                }
                Err(why) if reports.is_empty() && matches!(why, ToolError::Fatal(_)) => {
                    return Err(why);
                }
                Err(why) => reports.push(format!("({}) not opened: {why}", kind.name())),
            }
        }
        Ok(ToolOutput {
            report: reports.join("\n"),
            state: (!states.is_empty()).then(|| states.join("\n\n")),
            image,
        })
    }

    fn close(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let tabs: Vec<&str> = match arguments.get("tabs").and_then(Value::as_array) {
            Some(tabs) => tabs.iter().filter_map(Value::as_str).collect(),
            None => arguments
                .get("tab")
                .and_then(Value::as_str)
                .into_iter()
                .collect(),
        };
        if tabs.is_empty() {
            return Err(failed(
                "`tab` or `tabs` is needed: ids from `open` or `list`",
            ));
        }
        let mut said = vec![];
        for tab in tabs {
            let done = self.tab(Some(tab)).and_then(|(_, kind, id)| kind.close(id));
            said.push(match done {
                Ok(done) => format!("{tab}: {done}"),
                Err(why) => format!("{tab}: not closed: {why}"),
            });
        }
        Ok(said.join("\n").into())
    }

    /// The tabs: the window's own list where it shows, else each kind's.
    fn list(&self) -> String {
        let tabs: Vec<WindowTab> = match self.window.and_then(|window| window.tabs()) {
            Some(tabs) => tabs,
            None => (self.kinds.iter())
                .flat_map(|kind| {
                    let tabs = kind.tabs().into_iter();
                    tabs.map(|(id, title, detail)| WindowTab {
                        id: format!("{}{id}", kind.letter()),
                        kind: kind.name().into(),
                        title,
                        detail,
                        shown: false,
                        acted_on: false,
                    })
                })
                .collect(),
        };
        if tabs.is_empty() {
            return "No tabs are open.".into();
        }
        let s = if tabs.len() == 1 { "" } else { "s" };
        let mut out = vec![format!("{} tab{s}:", tabs.len())];
        for tab in tabs {
            let mut line = format!("{}  {}  {}", tab.id, tab.kind, tab.title);
            if !tab.detail.is_empty() && tab.detail != tab.title {
                line.push_str(&format!(" — {}", tab.detail));
            }
            let marks: Vec<&str> = [
                (tab.shown, "shown"),
                (tab.acted_on, "the browser acts here"),
            ]
            .into_iter()
            .filter_map(|(on, mark)| on.then_some(mark))
            .collect();
            if !marks.is_empty() {
                line.push_str(&format!("  ({})", marks.join(", ")));
            }
            out.push(line);
        }
        out.join("\n")
    }

    /// The tab `tab` names: its id as given, its kind, and its number among the kind's.
    fn tab(&self, tab: Option<&str>) -> Result<(String, &dyn Kind, u32), ToolError> {
        let tab = tab.ok_or_else(|| failed("`tab` is needed: an id from `open` or `list`"))?;
        let mut chars = tab.trim().chars();
        let letter = chars.next();
        let id = chars.as_str().parse::<u32>().ok();
        let kind = self.kinds.iter().find(|kind| Some(kind.letter()) == letter);
        match (kind, id) {
            (Some(kind), Some(id)) => Ok((tab.trim().to_string(), *kind, id)),
            _ => Err(failed(&format!(
                "there is no tab {tab}: ids are like w1 or t2"
            ))),
        }
    }

    fn names(&self) -> String {
        let names: Vec<&str> = self.kinds.iter().map(|kind| kind.name()).collect();
        names.join(", ")
    }
}

/// What the helper did, step by step, and how it ended; the values it never typed.
fn helper(done: &Report, values: &[String]) -> String {
    let outcome = match &done.outcome {
        Outcome::Done { probability } => format!("done ({:.0}% sure)", probability * 100.0),
        Outcome::Blocked => "blocked: nothing in the tab could make progress".into(),
        Outcome::Exhausted => "out of actions".into(),
        Outcome::Stopped(why) | Outcome::Unreachable(why) => format!("stopped: {why}"),
    };
    let mut out = format!("### Helper: {outcome}");
    for (i, step) in done.steps.iter().enumerate() {
        out.push_str(&format!("\n{}. {step}", i + 1));
    }
    let typed = |value: &String| (done.steps.iter()).any(|s| s.contains(&format!("\"{value}\"")));
    let unused: Vec<String> = (values.iter())
        .filter(|value| !typed(value))
        .map(|value| format!("\"{value}\""))
        .collect();
    if !unused.is_empty() {
        out.push_str(&format!("\nValues never typed: {}.", unused.join(", ")));
    }
    out
}

fn failed(why: &str) -> ToolError {
    ToolError::Failed(why.into())
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
