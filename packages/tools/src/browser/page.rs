//! The browser window and the page it shows, as a `View`: the window opens on the first address
//! it is sent to, and later addresses load in it. The browser's own toolbar (Back, Reload, page
//! size) is part of the view, so the hands can use it like any control.

use std::sync::Mutex;
use std::time::Duration;

use contracts::view::{Action, Capture, Observation, View, ViewError, ViewResult};

use super::{Browser, Driver, PageText, Viewport, WebviewDriver, Window, toolbar};

/// How web pages behave where it is not obvious: forms, suggestions, date pickers, results.
const CONVENTIONS: &str = "A form sends only once its required fields are filled and its Search, \
Submit or Done button is pressed. A typed search query usually needs its matching suggestion \
picked from the list that appears. A date picker takes the date and then its confirmation (Done, \
Apply or OK). A result opens by its link; a matching link is not yet the result. In a quiz or a \
form, the answer or option is picked before Next, Continue or Submit.";

/// How long a page may take to become usable before it is used anyway.
const LOAD_TIMEOUT: Duration = Duration::from_secs(20);
/// Characters of the document an expectation is judged on.
const CONTENT_LIMIT: usize = 4000;

pub struct Page {
    browser: Browser,
    headless: bool,
    /// The page size the toolbar shows (`desktop`, `tablet`, `mobile`).
    size: Mutex<String>,
    driver: Mutex<Option<WebviewDriver>>,
}

impl Page {
    pub fn new(browser: Browser, headless: bool) -> Self {
        Self {
            browser,
            headless,
            size: Mutex::new("desktop".into()),
            driver: Mutex::new(None),
        }
    }

    /// Load `url`, opening the window on it the first time; `title` names the window.
    pub fn open(&self, url: &str, title: &str) -> ViewResult<()> {
        let mut driver = self.driver.lock().unwrap();
        if let Some(open) = driver.as_ref() {
            return open.navigate(url);
        }
        let opened = self.browser.open(Window {
            url: url.to_string(),
            title: title.to_string(),
            headless: self.headless,
            viewport: Viewport::DESKTOP,
        })?;
        // A slow page is used as it is: it says it is busy until it settles.
        let _ = opened.wait_loaded(LOAD_TIMEOUT);
        *driver = Some(opened);
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.driver.lock().unwrap().is_some()
    }

    /// The open window's driver, for what only a browser has (scripts, keys, the whole text).
    pub fn driver(&self) -> ViewResult<WebviewDriver> {
        self.driver
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| ViewError::Unavailable("the browser window is not open".into()))
    }

    /// The document's readable text from `offset`, at most `limit` characters.
    pub fn read(&self, offset: usize, limit: usize) -> ViewResult<PageText> {
        self.driver()?.read(offset, limit)
    }

    /// A toolbar control: back, reload, or a page size.
    fn toolbar(&self, action: &Action) -> ViewResult<()> {
        let driver = self.driver()?;
        match action {
            Action::Click { reference } if reference == toolbar::BACK => {
                driver.evaluate("history.back()").map(|_| ())
            }
            Action::Click { reference } if reference == toolbar::RELOAD => {
                driver.evaluate("location.reload()").map(|_| ())
            }
            Action::Select { value, .. } => self.resize(value),
            _ => Err(ViewError::Refused(
                "a toolbar control is clicked, or the page size selected".into(),
            )),
        }
    }

    /// Reframe the page to a toolbar size (`desktop`, `tablet`, `mobile`).
    pub fn resize(&self, size: &str) -> ViewResult<()> {
        let (width, height) = toolbar::size(size)
            .ok_or_else(|| ViewError::Refused(format!("no page size {size}")))?;
        self.driver()?.resize(width, height)?;
        *self.size.lock().unwrap() = size.to_string();
        Ok(())
    }
}

impl View for Page {
    fn conventions(&self) -> &str {
        CONVENTIONS
    }

    fn observe(&self) -> ViewResult<Observation> {
        let mut view = Observation::from(self.driver()?.observe()?);
        let controls = toolbar::controls(&self.size.lock().unwrap());
        view.outline = format!("{}\n{}", toolbar::outline(&controls), view.outline);
        view.elements.splice(0..0, controls);
        Ok(view)
    }

    fn act(&self, epoch: &str, action: &Action) -> ViewResult<()> {
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

    fn capture(&self) -> ViewResult<Option<Capture>> {
        let bytes = self.driver()?.capture()?;
        Ok(Some(Capture {
            mime: "image/png",
            bytes,
        }))
    }

    fn content(&self) -> ViewResult<String> {
        self.read(0, CONTENT_LIMIT).map(|text| text.text)
    }
}
