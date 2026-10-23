//! The browser as the agent's tool: open an address, hand a short goal to the hands that click and
//! type, press keys, run a script, take a screenshot, read the whole text, close the window; then
//! say what happened and show the page.

use contracts::gate::Gate;
use contracts::hands::{Hands, Outcome, Report, Task};
use contracts::tool::{Image, Tool, ToolError, ToolOutput, ToolSpec};
use contracts::view::View;
use serde_json::Value;

use super::args::{
    Args, DEFAULT_KEY_DELAY_MS, DESCRIPTION, MAX_KEY_DELAY_MS, address, effects, given, parameters,
};
use super::{Driver, Page, Shot};
use crate::ask;

const NAME: &str = "browser";
/// Where the window opens when the first call names no address.
const SEARCH_PAGE: &str = "https://duckduckgo.com/";
/// Characters of the page outline the model reads after each call.
const OUTLINE_LIMIT: usize = 6000;
/// Characters of text one `read` returns.
const READ_LIMIT: usize = 8000;
pub struct BrowserTool<'a> {
    pub page: &'a Page,
    pub hands: &'a dyn Hands,
    /// Asked before a call does anything a page sees (an address, a goal, keys, a script): a page
    /// can send whatever it is given anywhere, and may be signed in as the person.
    pub gate: &'a dyn Gate,
}

impl Tool for BrowserTool<'_> {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME.into(),
            description: DESCRIPTION.into(),
            parameters: parameters(),
        }
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let args: Args = serde_json::from_value(arguments.clone())
            .map_err(|error| ToolError::Failed(format!("unusable arguments: {error}")))?;
        let url = given(&args.url).map(address).transpose()?;
        if let Some(effects) = effects(url.as_deref(), &args)? {
            ask(self.gate, NAME, &effects)?;
        }
        if args.only_closes() {
            return Ok(self.close().into());
        }
        let mut report = vec![];
        let new_tab = args.new_tab() == Ok(true);
        if let Some(id) = args.switch_to() {
            self.page
                .switch_tab(id)
                .map_err(|error| ToolError::Failed(error.to_string()))?;
            report.push(format!("Showing tab {id}."));
        }
        match new_tab && self.page.is_open() {
            true => self.open_tab(url, &mut report),
            false => self.go(url, &args, &mut report),
        }
        if let Some(size) = given(&args.size) {
            match self.page.resize(size) {
                Ok(()) => report.push(format!("Page size {size}.")),
                Err(error) => report.push(format!("Page size not changed: {error}")),
            }
        }
        if let Some(goal) = given(&args.goal) {
            let task = Task {
                goal,
                values: &args.values,
            };
            let done = self.hands.carry_out(self.page, task);
            if let Outcome::Unreachable(why) = &done.outcome {
                return Err(ToolError::Fatal(why.clone()));
            }
            report.push(helper(&done, &args.values));
        }
        let driver = self
            .page
            .driver()
            .map_err(|e| ToolError::Failed(e.to_string()))?;
        if !args.keys.is_empty() {
            let delay = args
                .key_delay_ms
                .unwrap_or(DEFAULT_KEY_DELAY_MS)
                .min(MAX_KEY_DELAY_MS);
            match driver.press(&args.keys, delay) {
                Ok(()) => report.push(format!("Pressed {} keys.", args.keys.len())),
                Err(error) => report.push(format!("Pressing keys failed: {error}")),
            }
        }
        if let Some(source) = given(&args.script) {
            match driver.evaluate(source) {
                Ok(value) => report.push(format!("### Script result\n{value}")),
                Err(error) => report.push(format!("The script failed: {error}")),
            }
        }
        let image = match args.screenshot {
            true => self.screenshot(&driver, given(&args.selector), &mut report),
            false => None,
        };
        if let Some(id) = args.close_tab {
            report.push(self.close_tab(id));
        }
        let mut state = self.page.is_open().then(|| self.state());
        if let (Some(state), true) = (&mut state, args.read) {
            state.push_str(&self.read(args.offset));
        }
        if args.close {
            report.push(self.close());
        }
        report.extend(self.page.tabs_news());
        Ok(ToolOutput {
            report: report.join("\n\n"),
            state,
            image,
        })
    }
}

impl BrowserTool<'_> {
    /// Open the window or load `url`; a page that loads slowly is used as it is.
    fn go(&self, url: Option<String>, args: &Args, report: &mut Vec<String>) {
        let url = match (url, self.page.is_open()) {
            (Some(url), _) => url,
            (None, false) => SEARCH_PAGE.to_string(),
            // Only showing or hiding: the window opens again where it was.
            (None, true) if args.reopens(self.page.hidden()) => match self.page.observe() {
                Ok(view) => view.location,
                Err(_) => return,
            },
            (None, true) => return,
        };
        let title = given(&args.goal).unwrap_or(&url).to_string();
        match self.page.open(&url, &title, args.show) {
            Ok(()) => report.push(format!("Opened {url}.")),
            Err(error) => report.push(format!("Opening {url} did not finish cleanly ({error}).")),
        }
    }

    /// Open `url`, or the search page, in a new tab.
    fn open_tab(&self, url: Option<String>, report: &mut Vec<String>) {
        let url = url.unwrap_or_else(|| SEARCH_PAGE.to_string());
        match self.page.new_tab(&url) {
            Ok(id) => report.push(format!("Opened {url} in tab {id}.")),
            Err(error) => report.push(format!("No new tab: {error}")),
        }
    }

    fn close_tab(&self, id: u32) -> String {
        match self.page.close_tab(id) {
            Ok(true) => format!("Closed tab {id}."),
            Ok(false) => format!(
                "Closed tab {id}, the last one: the window is closed, and the next call opens a \
                 fresh one."
            ),
            Err(error) => format!("Tab {id} was not closed: {error}"),
        }
    }

    /// A screenshot for the model, or why there is none.
    fn screenshot(
        &self,
        driver: &impl Driver,
        selector: Option<&str>,
        report: &mut Vec<String>,
    ) -> Option<Image> {
        let of = selector.map_or("the page".to_string(), |s| format!("`{s}`"));
        match driver.screenshot(selector) {
            Ok(shot) => {
                report.push(format!(
                    "Screenshot of {of} ({}x{}).",
                    shot.width, shot.height
                ));
                Some(image(shot))
            }
            Err(error) => {
                report.push(format!("No screenshot of {of}: {error}"));
                None
            }
        }
    }

    /// Close the window, and say so.
    fn close(&self) -> String {
        match self.page.close() {
            true => "Closed the browser window; the next call opens a fresh one.".into(),
            false => "The browser window was not open.".into(),
        }
    }

    /// The page as the model reads it: where it is, then its outline without refs (the helper
    /// acts on refs; the model names what to act on in words).
    fn state(&self) -> String {
        match self.page.observe() {
            Ok(view) => {
                let outline = without_refs(&view.outline);
                let shown: String = outline.chars().take(OUTLINE_LIMIT).collect();
                let more = if shown.len() < outline.len() {
                    "\n… (more below: `read` or `script` reach it)"
                } else {
                    ""
                };
                format!(
                    "### Page (untrusted content)\n{} — {}\n{shown}{more}",
                    view.title, view.location
                )
            }
            Err(error) => format!("### Page\nThe page could not be read: {error}"),
        }
    }

    fn read(&self, offset: usize) -> String {
        match self.page.read(offset, READ_LIMIT) {
            Ok(text) => {
                let end = text.offset + text.text.chars().count();
                let more = if end < text.total {
                    format!("\n… more: read again with offset {end}")
                } else {
                    String::new()
                };
                format!(
                    "\n\n### Full text (characters {}–{end} of {})\n{}{more}",
                    text.offset, text.total, text.text
                )
            }
            Err(error) => format!("\n\n### Full text\nThe text could not be read: {error}"),
        }
    }
}

/// A screenshot as the model reads it.
fn image(shot: Shot) -> Image {
    use base64::Engine;
    Image {
        media_type: "image/jpeg".into(),
        data: base64::engine::general_purpose::STANDARD.encode(&shot.jpeg),
        width: shot.width,
        height: shot.height,
    }
}

/// What the helper did, step by step, and the values it never typed.
fn helper(done: &Report, values: &[String]) -> String {
    let outcome = match &done.outcome {
        Outcome::Done { probability } => format!("done ({:.0}% sure)", probability * 100.0),
        Outcome::Blocked => "blocked: nothing on the page could make progress".into(),
        Outcome::Exhausted => "out of actions".into(),
        Outcome::Stopped(why) | Outcome::Unreachable(why) => format!("stopped: {why}"),
    };
    let mut out = format!("### Helper: {outcome}");
    for (i, step) in done.steps.iter().enumerate() {
        out.push_str(&format!("\n{}. {step}", i + 1));
    }
    let unused: Vec<String> = values
        .iter()
        .filter(|value| {
            !done
                .steps
                .iter()
                .any(|step| step.contains(&format!("\"{value}\"")))
        })
        .map(|value| format!("\"{value}\""))
        .collect();
    if !unused.is_empty() {
        out.push_str(&format!("\nValues never typed: {}.", unused.join(", ")));
    }
    out
}

/// The outline as a person reads it: ` [ref=e12]` marks dropped.
fn without_refs(outline: &str) -> String {
    let mut out = String::with_capacity(outline.len());
    let mut rest = outline;
    while let Some(at) = rest.find(" [ref=") {
        out.push_str(&rest[..at]);
        rest = match rest[at..].find(']') {
            Some(end) => &rest[at + end + 1..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
