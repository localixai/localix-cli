//! The tabs by id: opening several at once, closing them, listing them, and the tab an id names.

use contracts::tool::{ToolError, ToolOutput};
use serde_json::Value;

use super::{Kind, Opened, WindowTab, WindowTool, failed};

impl WindowTool<'_> {
    /// Open each tab asked for; one that fails says why, and the others still open.
    #[expect(
        clippy::needless_collect,
        reason = "every tab's thread starts before the first is joined: a lazy iterator would open them one by one"
    )]
    pub(super) fn open(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
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
                .map(|(kind, spec)| {
                    let thread = std::thread::Builder::new().name("llx-tab-open".into());
                    thread.spawn_scoped(scope, move || kind.open(spec))
                })
                .collect();
            let joined = opening.into_iter().map(|started| match started {
                Ok(thread) => thread
                    .join()
                    .unwrap_or_else(|_| Err(failed("opening it failed"))),
                Err(why) => Err(failed(&format!("it could not start (no thread: {why})"))),
            });
            joined.collect()
        });
        let (mut reports, mut states, mut image) = (vec![], vec![], None);
        for (kind, opened) in kinds.into_iter().zip(opened) {
            match opened {
                Ok(opened) => {
                    let tab = format!("{}{}", kind.letter(), opened.id);
                    if let Some(window) = self.window {
                        window.opened(&tab);
                    }
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

    pub(super) fn close(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
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
    pub(super) fn list(&self) -> String {
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
                        group: String::new(),
                    })
                })
                .collect(),
        };
        if tabs.is_empty() {
            return "No tabs are open.".into();
        }
        // A tab names its group only beside another group's: one project reads as it always has.
        let grouped = tabs.iter().any(|tab| tab.group != tabs[0].group);
        let s = if tabs.len() == 1 { "" } else { "s" };
        let mut out = vec![format!("{} tab{s}:", tabs.len())];
        for tab in tabs {
            let mut line = format!("{}  {}  {}", tab.id, tab.kind, tab.title);
            if !tab.detail.is_empty() && tab.detail != tab.title {
                line.push_str(&format!(" — {}", tab.detail));
            }
            let group = format!("group {}", tab.group);
            let marks: Vec<&str> = [
                (grouped, group.as_str()),
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
    pub(super) fn tab(&self, tab: Option<&str>) -> Result<(String, &dyn Kind, u32), ToolError> {
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
