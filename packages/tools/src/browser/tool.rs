//! The browser as a `Tool`: the window opens on the first goal that needs it, on the page
//! the goal names (or the start page, or a search page), and later goals continue where it is
//! unless they name another site.

use std::sync::Mutex;
use std::time::Duration;

use agent::tool::{Action, Capture, Observation, Tool, ToolError, ToolResult};

use super::start::{SEARCH_PAGE, url_in};
use super::{Browser, Driver, Viewport, WebviewDriver, Window, toolbar};

/// How web pages behave where it is not obvious: forms, suggestions, date pickers, results.
const CONVENTIONS: &str = "A form sends only once its required fields are filled and its Search, \
Submit or Done button is pressed. A typed search query usually needs its matching suggestion \
picked from the list that appears. A date picker takes the date and then its confirmation (Done, \
Apply or OK). A result opens by its link; a matching link is not yet the result. In a quiz or a \
form, the answer or option is picked before Next, Continue or Submit.";

/// How long a page may take to become usable before the goal starts anyway.
const LOAD_TIMEOUT: Duration = Duration::from_secs(20);
/// Characters of the document an expectation is judged on.
const CONTENT_LIMIT: usize = 4000;

pub struct BrowserTool {
    browser: Browser,
    headless: bool,
    /// The page size the toolbar shows (`desktop`, `tablet`, `mobile`).
    size: Mutex<String>,
    driver: Mutex<Option<WebviewDriver>>,
}

impl BrowserTool {
    pub fn new(browser: Browser, headless: bool) -> Self {
        Self {
            browser,
            headless,
            size: Mutex::new("desktop".into()),
            driver: Mutex::new(None),
        }
    }

    /// A toolbar control: back, reload, or a page size.
    fn toolbar(&self, action: &Action) -> ToolResult<()> {
        let driver = self.driver()?;
        match action {
            Action::Click { reference } if reference == toolbar::BACK => {
                driver.evaluate("history.back()").map(|_| ())
            }
            Action::Click { reference } if reference == toolbar::RELOAD => {
                driver.evaluate("location.reload()").map(|_| ())
            }
            Action::Select { value, .. } => {
                let (width, height) = toolbar::size(value)
                    .ok_or_else(|| ToolError::Refused(format!("no page size {value}")))?;
                driver.resize(width, height)?;
                *self.size.lock().unwrap() = value.clone();
                Ok(())
            }
            _ => Err(ToolError::Refused(
                "a toolbar control is clicked, or the page size selected".into(),
            )),
        }
    }

    /// The open window's driver, for what only a browser has (render checks, scripts).
    pub fn driver(&self) -> ToolResult<WebviewDriver> {
        self.driver
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| ToolError::Unavailable("the browser window is not open".into()))
    }
}

impl Tool for BrowserTool {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "A web browser: open sites, search the web, read pages, fill in and submit forms, click links \
         and buttons, take online tests."
    }

    fn conventions(&self) -> &str {
        CONVENTIONS
    }

    fn start(&self, goal: &str, at: Option<&str>) -> ToolResult<()> {
        // A web address another tool left off at comes first, then one the goal names.
        let named = at.and_then(url_in).or_else(|| url_in(goal));
        let mut driver = self.driver.lock().unwrap();
        match driver.as_ref() {
            Some(open) => match named {
                Some(url) => open.navigate(&url),
                None => Ok(()),
            },
            None => {
                let url = named.unwrap_or_else(|| SEARCH_PAGE.into());
                let opened = self.browser.open(Window {
                    url,
                    title: goal.to_string(),
                    headless: self.headless,
                    viewport: Viewport::DESKTOP,
                })?;
                // A slow page is used as it is: it says it is busy until it settles.
                let _ = opened.wait_loaded(LOAD_TIMEOUT);
                *driver = Some(opened);
                Ok(())
            }
        }
    }

    fn observe(&self) -> ToolResult<Observation> {
        let mut view = Observation::from(self.driver()?.observe()?);
        let controls = toolbar::controls(&self.size.lock().unwrap());
        view.outline = format!("{}\n{}", toolbar::outline(&controls), view.outline);
        view.elements.splice(0..0, controls);
        Ok(view)
    }

    fn act(&self, epoch: &str, action: &Action) -> ToolResult<()> {
        let reference = match action {
            Action::Click { reference }
            | Action::Type { reference, .. }
            | Action::Select { reference, .. } => Some(reference.as_str()),
            _ => None,
        };
        match reference {
            Some(reference) if toolbar::owns(reference) => self.toolbar(action),
            _ => self.driver()?.act(epoch, action),
        }
    }

    fn capture(&self) -> ToolResult<Option<Capture>> {
        let bytes = self.driver()?.capture()?;
        Ok(Some(Capture {
            mime: "image/png",
            bytes,
        }))
    }

    fn content(&self) -> ToolResult<String> {
        self.driver()?.read(0, CONTENT_LIMIT).map(|text| text.text)
    }
}
