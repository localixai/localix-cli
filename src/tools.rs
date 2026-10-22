//! The planner's one tool, `browser`: open an address, hand the decision model a sub-goal, read the
//! page. Every result is plain text ending with the page as it now is.

use serde_json::{Value, json};

use crate::browser::{Driver, Observation};
use crate::decision::Decider;
use crate::goal::{GoalOutcome, GoalRun};
use crate::goal_ask::GoalStep;

pub const TOOL: &str = "browser";
const PAGE_TEXT: usize = 1500;
const PAGE_CONTROLS: usize = 100;
const READ_LIMIT: usize = 8000;

/// The planner's tools: `browser`, and `result` when the run has an output schema.
pub fn schemas(schema: Option<&Value>) -> Value {
    let mut tools = json!([{"type": "function", "function": {
        "name": TOOL,
        "description": "Drive the browser. Give `goal`: ONE short sub-goal that can be done on the current \
            page (fill and submit a form, pick a date, open a result, accept a banner). A fast decision \
            model clicks, selects, scrolls and types by itself, but types ONLY the exact strings in \
            `values`. Optionally open `url` first. `keys` presses keys on the page (games, shortcuts). `script` runs JavaScript in the page after the \
            goal (to read data the page holds, or to do what clicking cannot) and returns its value. \
            Set `read` to get the whole page's text instead of only what is on screen. Returns what \
            was done and the resulting page.",
        "parameters": {"type": "object", "properties": {
            "goal": {"type": "string", "description": "An action to perform, never a reading task (use script/read for that). Concrete, checkable sub-goal, e.g. 'Search one-way flights from Zurich to London on 20 Sep 2026'. Leave empty to only open `url` or `read`."},
            "values": {"type": "array", "items": {"type": "string"}, "description": "Exact strings to type into fields, in the form the site expects."},
            "url": {"type": "string", "description": "Address to open before the goal, e.g. a site or a search URL."},
            "viewport": {"type": "string", "description": "Reframe the page first: desktop, tablet, mobile, or WIDTHxHEIGHT (layout only; the user agent is the one the run started with)."},
            "keys": {"type": "array", "items": {"type": "string"}, "description": "Keys to press on the page after the goal, in order, e.g. [\"ArrowUp\", \"ArrowLeft\"] for a game, [\"Escape\"], [\"Ctrl+A\"]. Up to 500."},
            "key_delay_ms": {"type": "integer", "minimum": 0, "maximum": 2000, "description": "Pause between keys (default 60), for games that animate each move."},
            "script": {"type": "string", "description": "JavaScript run in the page: an expression, or statements whose last value is returned (await works in an expression)."},
            "read": {"type": "boolean", "description": "Return the full page text (after the goal and script, if any)."},
            "offset": {"type": "integer", "minimum": 0, "description": "Where to continue reading a long page."}
        }}
    }}]);
    if let (Some(schema), Some(list)) = (schema, tools.as_array_mut()) {
        list.push(crate::structured::tool(schema));
    }
    tools
}

/// The page as the planner reads it: where it is, what it says, what can be used.
pub fn describe_page(page: &Observation) -> String {
    let text: String = page.text.chars().take(PAGE_TEXT).collect();
    let more = if page.text.chars().count() > PAGE_TEXT {
        " …"
    } else {
        ""
    };
    let mut out = format!(
        "Page: {} — {}\nVisible text:\n{text}{more}\n\n",
        page.title, page.url
    );
    if !page.fields.is_empty() {
        let filled: Vec<String> = page
            .fields
            .iter()
            .map(|f| format!("{} = \"{}\"", f.name, f.value))
            .collect();
        out.push_str(&format!("Filled in: {}\n\n", filled.join(" · ")));
    }
    out.push_str("Controls:\n");
    // An open dialog (a date picker, a filter panel) is what the page is about now: its controls first.
    let (dialog, rest): (Vec<_>, Vec<_>) = page.elements.iter().partition(|element| element.dialog);
    for element in dialog.into_iter().chain(rest).take(PAGE_CONTROLS) {
        let dialog = if element.dialog { " (in dialog)" } else { "" };
        out.push_str(&format!(
            "[{}] {}{dialog}\n",
            element.reference,
            element.describe()
        ));
    }
    let hidden = page.elements.len().saturating_sub(PAGE_CONTROLS) + page.omitted;
    if hidden > 0 {
        out.push_str(&format!("… {hidden} more controls below or off screen\n"));
    }
    if page.scroll.y + i64::from(page.viewport.h) < page.scroll.height - 2 {
        out.push_str("The page continues below.\n");
    }
    out
}

/// What a tool call did, for the conversation and for the usage report.
pub struct ToolOutcome {
    pub content: String,
    pub goal: Option<GoalOutcome>,
}

pub struct Tools<'a> {
    pub driver: &'a dyn Driver,
    pub decider: &'a dyn Decider,
    pub on_step: &'a dyn Fn(&GoalStep),
    pub max_actions: usize,
}

fn string(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl Tools<'_> {
    pub fn run(&self, name: &str, args: &Value) -> ToolOutcome {
        if name != TOOL {
            return ToolOutcome {
                content: format!("Unknown tool {name}; the only tool is {TOOL}."),
                goal: None,
            };
        }
        let mut content = String::new();
        if let Some(url) = string(args, "url") {
            let url = if url.contains("://") {
                url
            } else {
                format!("https://{url}")
            };
            if let Err(error) = self.driver.navigate(&url) {
                content.push_str(&format!(
                    "Opening {url} did not finish cleanly ({error}).\n"
                ));
            }
        }
        if let Some(text) = string(args, "viewport") {
            match crate::browser::Viewport::parse(&text).and_then(|v| {
                self.driver
                    .resize(v.width, v.height)
                    .map(|_| v)
                    .map_err(|e| e.to_string())
            }) {
                Ok(viewport) => content.push_str(&format!("Viewport {}.\n", viewport.label())),
                Err(error) => content.push_str(&format!("Viewport not changed: {error}\n")),
            }
        }
        let mut goal = None;
        let mut page = None;
        if let Some(text) = string(args, "goal") {
            let values: Vec<String> = args
                .get("values")
                .and_then(Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let run = GoalRun {
                driver: self.driver,
                decider: self.decider,
                on_step: self.on_step,
                max_actions: self.max_actions,
            };
            let outcome = run.run(&text, &values);
            content.push_str(&format!(
                "The decision model took {} steps; stopped: {}.\n",
                outcome.steps.len(),
                outcome.stop
            ));
            for (i, step) in outcome.steps.iter().enumerate() {
                content.push_str(&format!("{}. {}\n", i + 1, step.trace_line()));
            }
            let typed: Vec<&str> = outcome
                .steps
                .iter()
                .filter_map(|step| step.value.as_deref())
                .collect();
            let unused: Vec<&String> = values
                .iter()
                .filter(|value| !typed.contains(&value.as_str()))
                .collect();
            if !unused.is_empty() {
                let list: Vec<String> = unused.iter().map(|value| format!("\"{value}\"")).collect();
                content.push_str(&format!("Values never typed: {}.\n", list.join(", ")));
            }
            content.push_str("Check the page yourself; DONE is not proof.\n");
            page = outcome.page.clone();
            goal = Some(outcome);
        }
        let keys: Vec<String> = args
            .get("keys")
            .and_then(Value::as_array)
            .map(|keys| {
                keys.iter()
                    .filter_map(|k| k.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if !keys.is_empty() {
            let delay = args
                .get("key_delay_ms")
                .and_then(Value::as_u64)
                .unwrap_or(60)
                .min(2000);
            match self.driver.press(&keys, delay) {
                Ok(()) => content.push_str(&format!("Pressed {} keys.\n", keys.len())),
                Err(error) => content.push_str(&format!("Pressing keys failed: {error}\n")),
            }
            page = None;
        }
        if let Some(source) = string(args, "script") {
            match self.driver.evaluate(&source) {
                Ok(result) => content.push_str(&format!("Script result:\n{result}\n\n")),
                Err(error) => content.push_str(&format!("The script failed: {error}\n\n")),
            }
            page = None;
        }
        if args.get("read").and_then(Value::as_bool).unwrap_or(false) {
            let offset = args.get("offset").and_then(Value::as_u64).unwrap_or(0) as usize;
            content.push_str(&self.read(offset));
        }
        let page = page.map(Ok).unwrap_or_else(|| self.driver.observe());
        content.push_str(&match page {
            Ok(page) => describe_page(&page),
            Err(error) => format!("The page could not be read: {error}\n"),
        });
        ToolOutcome { content, goal }
    }

    fn read(&self, offset: usize) -> String {
        match self.driver.read(offset, READ_LIMIT) {
            Ok(text) => {
                let end = text.offset + text.text.chars().count();
                let more = if end < text.total {
                    format!("\n… more: read again with offset {end}")
                } else {
                    String::new()
                };
                format!(
                    "Full text, characters {}–{end} of {}:\n{}{more}\n\n",
                    text.offset, text.total, text.text
                )
            }
            Err(error) => format!("The text could not be read: {error}\n"),
        }
    }
}
