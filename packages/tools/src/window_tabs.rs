//! The tabs by id: opening several at once, closing them, listing them, and the tab an id names.

use std::fmt::Write as _;

use contracts::tool::{ToolError, ToolOutput};
use serde_json::{Value, json};

use super::{Kind, Opened, WindowTab, WindowTool, failed};

/// What a goal given with `open` did in the tab it opened.
type Worked = Result<ToolOutput, ToolError>;

impl<'a> WindowTool<'a> {
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
            let named = spec.get("kind").and_then(Value::as_str);
            let kind = match named {
                Some(named) => self.kinds.iter().copied().find(|k| k.name() == named),
                None => self.implied(spec),
            };
            let Some(kind) = kind else {
                return Err(failed(&self.no_kind(named, spec)));
            };
            kinds.push(kind);
        }
        // Every tab of the call opens at once, and its goal, if it has one, works in it as soon
        // as it is open; the reports keep the order they were asked in.
        let opened: Vec<Result<(Opened, Option<Worked>), ToolError>> =
            std::thread::scope(|scope| {
                let opening: Vec<_> = (kinds.iter().zip(&asked))
                    .map(|(kind, spec)| {
                        let thread = std::thread::Builder::new().name("llx-tab-open".into());
                        thread.spawn_scoped(scope, move || {
                            let opened = kind.open(spec)?;
                            let tab = format!("{}{}", kind.letter(), opened.id);
                            let worked = self.goal_in_opened(&tab, spec);
                            Ok((opened, worked))
                        })
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
                Ok((opened, worked)) => {
                    let tab = format!("{}{}", kind.letter(), opened.id);
                    if let Some(window) = self.window {
                        window.opened(&tab);
                    }
                    reports.push(format!("{tab} ({}): {}", kind.name(), opened.output.report));
                    let mut state = opened.output.state;
                    match worked {
                        Some(Ok(worked)) => {
                            reports.push(worked.report);
                            state = worked.state.or(state);
                        }
                        Some(Err(why)) => reports.push(format!("{tab}: the goal failed: {why}")),
                        None => {}
                    }
                    if let Some(state) = state {
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

    /// The goal `spec` gives a tab just opened as `tab`, carried out in it; None without one.
    fn goal_in_opened(&self, tab: &str, spec: &Value) -> Option<Worked> {
        let goal =
            (spec.get("goal").and_then(Value::as_str)).filter(|goal| !goal.trim().is_empty())?;
        let values = spec.get("values").cloned().unwrap_or(Value::Null);
        Some(self.interact(&json!({"tab": tab, "goal": goal, "values": values})))
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
                let _ = write!(line, " — {}", tab.detail);
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
                let _ = write!(line, "  ({})", marks.join(", "));
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

    /// The kind a tab with no `kind` is of: the one kind with a field of its own among those it
    /// gives (`url`, `command`), so a call that left `kind` out still opens what it names.
    fn implied(&self, spec: &Value) -> Option<&'a dyn Kind> {
        let given = |field: &str| spec.get(field).is_some();
        let owns = |kind: &&'a dyn Kind| {
            kind.fields().iter().any(|(field, _)| {
                given(field)
                    && (self.kinds.iter())
                        .filter(|other| other.fields().iter().any(|(f, _)| f == field))
                        .count()
                        == 1
            })
        };
        let mut implied = self.kinds.iter().copied().filter(|kind| owns(kind));
        match (implied.next(), implied.next()) {
            (Some(kind), None) => Some(kind),
            (Some(_), Some(_)) | (None, _) => None,
        }
    }

    /// Why a tab has no kind, with what the call gave, so the next call can mend it.
    fn no_kind(&self, named: Option<&str>, spec: &Value) -> String {
        let gave = match named {
            Some(named) => format!("`kind` \"{named}\" is none of them"),
            None => {
                let fields = spec
                    .as_object()
                    .into_iter()
                    .flat_map(|fields| fields.keys());
                let fields: Vec<String> = (fields.filter(|field| *field != "action"))
                    .map(|field| format!("`{field}`"))
                    .collect();
                match fields.is_empty() {
                    true => "the call gave no `kind` and no field".into(),
                    false => format!("the call gave no `kind`, only {}", fields.join(", ")),
                }
            }
        };
        format!("`kind` is one of: {}; {gave}", self.names())
    }

    fn names(&self) -> String {
        let names: Vec<&str> = self.kinds.iter().map(|kind| kind.name()).collect();
        names.join(", ")
    }
}
