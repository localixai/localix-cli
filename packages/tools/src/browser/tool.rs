//! The browser as the agent's tool: open an address, hand a short goal to the hands that click and
//! type, press keys, run a script, take a screenshot, read the whole text, close the window; then
//! say what happened and show the page.

use contracts::gate::Gate;
use contracts::tool::{Image, Tool, ToolError, ToolOutput, ToolSpec};
use contracts::view::View;
use serde_json::Value;
use std::time::Instant;

use super::args::{
    Args, DEFAULT_KEY_DELAY_MS, DEFAULT_WAIT_MS, DESCRIPTION, MAX_KEY_DELAY_MS, MAX_WAIT_MS,
    address, effects, given, parameters,
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
        if let Some(effects) = effects(url.as_deref(), &args) {
            ask(self.gate, NAME, "", &effects)?;
        }
        let mut report = vec![];
        match args.new_tab && self.page.is_open() {
            true => self.open_tab(url, &mut report),
            false => self.go(url, &mut report),
        }
        if let Some(size) = given(&args.size) {
            match self.page.resize(size) {
                Ok(()) => report.push(format!("Page size {size}.")),
                Err(error) => report.push(format!("Page size not changed: {error}")),
            }
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
        if let Some(text) = given(&args.wait_for) {
            let most = args.yield_ms.unwrap_or(DEFAULT_WAIT_MS).min(MAX_WAIT_MS);
            report.push(wait_for(&driver, text, most));
        }
        let image = match args.screenshot {
            true => Self::screenshot(&driver, given(&args.selector), &mut report),
            false => None,
        };
        if let Some(id) = args.close_tab {
            report.push(self.close_tab(id));
        }
        let mut state = self.page.is_open().then(|| self.state());
        if let (Some(state), true) = (&mut state, args.read) {
            state.push_str(&self.read(args.offset));
        }
        Ok(ToolOutput {
            report: report.join("\n\n"),
            state,
            image,
        })
    }
}

impl BrowserTool<'_> {
    /// Open the window or load `url`; a page that loads slowly is used as it is.
    fn go(&self, url: Option<String>, report: &mut Vec<String>) {
        let url = match (url, self.page.is_open()) {
            (Some(url), _) => url,
            (None, false) => SEARCH_PAGE.to_string(),
            (None, true) => return,
        };
        match self.page.open(&url, &url, None) {
            Ok(()) => report.push(format!("Opened {url}.")),
            Err(error) => report.push(format!("Opening {url} did not finish cleanly ({error}).")),
        }
    }

    /// Open `url`, or the search page, in a new tab.
    fn open_tab(&self, url: Option<String>, report: &mut Vec<String>) {
        let url = url.unwrap_or_else(|| SEARCH_PAGE.to_string());
        match self.page.driver().and_then(|driver| driver.new_tab(&url)) {
            Ok(id) => report.push(format!("Opened {url} in tab {id}.")),
            Err(error) => report.push(format!("No new tab: {error}")),
        }
    }

    fn close_tab(&self, id: u32) -> String {
        match self.page.driver().and_then(|driver| driver.close_tab(id)) {
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
        driver: &impl Driver,
        selector: Option<&str>,
        report: &mut Vec<String>,
    ) -> Option<Image> {
        let of = selector.map_or_else(|| "the page".to_string(), |s| format!("`{s}`"));
        match driver.screenshot(selector) {
            Ok(shot) => {
                report.push(format!(
                    "Screenshot of {of} ({}x{}).",
                    shot.width, shot.height
                ));
                Some(image(&shot))
            }
            Err(error) => {
                report.push(format!("No screenshot of {of}: {error}"));
                None
            }
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
        match self
            .page
            .driver()
            .and_then(|driver| driver.read(offset, READ_LIMIT))
        {
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
fn image(shot: &Shot) -> Image {
    use base64::Engine;
    Image {
        media_type: "image/jpeg".into(),
        data: base64::engine::general_purpose::STANDARD.encode(&shot.jpeg),
        width: shot.width,
        height: shot.height,
    }
}

/// The outline as a person reads it: ` [ref=e12]` marks dropped.
fn without_refs(outline: &str) -> String {
    let mut out = String::with_capacity(outline.len());
    let mut rest = outline;
    while let Some((before, marked)) = rest.split_once(" [ref=") {
        out.push_str(before);
        rest = marked.split_once(']').map_or("", |(_, after)| after);
    }
    out.push_str(rest);
    out
}

/// Waits until the page's text shows `text`, at most `most` ms, polled in the page on a timer
/// (animation frames stop in a window in the background); says whether it came and when.
fn wait_for(driver: &impl Driver, text: &str, most: u64) -> String {
    let wanted = Value::String(text.to_string());
    let source = format!(
        "const wanted = {wanted}; const until = performance.now() + {most}; \
         const shows = () => (document.body?.innerText ?? '').includes(wanted); \
         while (!shows() && performance.now() < until) await sleep(100); shows()"
    );
    let started = Instant::now();
    let seconds = |ms: u128| format!("{}.{}s", ms / 1000, ms % 1000 / 100);
    match driver.evaluate(&source).as_deref() {
        Ok("true") => format!(
            "\"{text}\" appeared after {}",
            seconds(started.elapsed().as_millis())
        ),
        Ok(_) => format!(
            "\"{text}\" did not appear within {}: the page may still be working; wait again, \
             or read what it shows",
            seconds(u128::from(most))
        ),
        Err(error) => format!("Waiting for \"{text}\" failed: {error}"),
    }
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
