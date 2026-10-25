//! The browser window and the page it shows, as a `View`: the window opens on the first address
//! it is sent to, and later addresses load in it. The browser's own toolbar (Back, Reload, page
//! size) is part of the view, so the hands can use it like any control; with several tabs open,
//! it also says which tab the view is, but the hands do not switch tabs: the agent does.

use std::sync::Mutex;
use std::time::Duration;

use contracts::view::{Action, Observation, View, ViewError, ViewResult};

use super::{Browser, Driver, Viewport, WebviewDriver, Window, toolbar};

/// How web pages behave where it is not obvious: forms, suggestions, date pickers, results.
const CONVENTIONS: &str = "A form sends only once its required fields are filled and its Search, \
Submit or Done button is pressed. A typed search query usually needs its matching suggestion \
picked from the list that appears. A date picker takes the date and then its confirmation (Done, \
Apply or OK). A result opens by its link; a matching link is not yet the result. In a quiz or a \
form, the answer or option is picked before Next, Continue or Submit.";

/// How long a page may take to become usable before it is used anyway.
const LOAD_TIMEOUT: Duration = Duration::from_secs(20);

pub struct Page {
    browser: Browser,
    /// Hidden unless a call asks to show it: the default the app chose for this run.
    headless: bool,
    /// Whether the open window is hidden; it is opened again when a call wants the other.
    hidden: Mutex<bool>,
    /// The page size the toolbar shows (`desktop`, `tablet`, `mobile`).
    size: Mutex<String>,
    driver: Mutex<Option<WebviewDriver>>,
}

impl Page {
    pub fn new(browser: Browser, headless: bool) -> Self {
        Self {
            browser,
            headless,
            hidden: Mutex::new(headless),
            size: Mutex::new("desktop".into()),
            driver: Mutex::new(None),
        }
    }

    /// Load `url`, opening the window on it the first time (and again after the person closed
    /// it); `title` names the window. `show` overrides the run's default for this call: a window
    /// open the other way is closed and opened again, on `url`.
    pub fn open(&self, url: &str, title: &str, show: Option<bool>) -> ViewResult<()> {
        let mut driver = crate::lock(&self.driver);
        let hidden = show.map_or(self.headless, |show| !show);
        let mut current = crate::lock(&self.hidden);
        if let Some(open) = driver.as_ref().filter(|driver| driver.is_open()) {
            if *current == hidden {
                return open.navigate(url);
            }
            closed(open);
        }
        // The window is open for someone else (another client of the daemon): this page works in
        // a tab of its own and leaves theirs as it is.
        if let Some(attached) = self.browser.attach() {
            attached
                .new_tab(url)
                .map_err(|error| ViewError::Unavailable(error.to_string()))?;
            *driver = Some(attached);
            return Ok(());
        }
        *current = hidden;
        let opened = self.browser.open(Window {
            url: url.to_string(),
            title: title.to_string(),
            headless: hidden,
            viewport: Viewport::DESKTOP,
        })?;
        // A slow page is used as it is: it says it is busy until it settles.
        let _ = opened.wait_loaded(LOAD_TIMEOUT);
        *driver = Some(opened);
        Ok(())
    }

    /// Close the window, as the agent asks (not the person: the task goes on); the next call that
    /// needs it opens it again. Returns whether one was open.
    pub fn close(&self) -> bool {
        let driver = crate::lock(&self.driver);
        match driver.as_ref().filter(|driver| driver.is_open()) {
            Some(open) => {
                closed(open);
                true
            }
            None => false,
        }
    }

    /// Whether the window is (or opens) hidden.
    pub fn hidden(&self) -> bool {
        *crate::lock(&self.hidden)
    }

    pub fn is_open(&self) -> bool {
        crate::lock(&self.driver)
            .as_ref()
            .is_some_and(WebviewDriver::is_open)
    }

    /// The open window's driver, for what only a browser has (scripts, keys, the whole text).
    pub fn driver(&self) -> ViewResult<WebviewDriver> {
        crate::lock(&self.driver)
            .clone()
            .filter(WebviewDriver::is_open)
            .ok_or_else(|| ViewError::Unavailable("the browser window is not open".into()))
    }

    /// The same window acting in tab `id`, whatever is shown: a page of its own to work in
    /// beside the others.
    pub fn pinned(&self, id: u32) -> Page {
        Page {
            browser: self.browser.clone(),
            headless: self.headless,
            hidden: Mutex::new(self.hidden()),
            size: Mutex::new(crate::lock(&self.size).clone()),
            driver: Mutex::new(crate::lock(&self.driver).as_ref().map(|d| d.on(id))),
        }
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
        *crate::lock(&self.size) = size.to_string();
        Ok(())
    }
}

/// Close `open` and wait, briefly, until it is.
#[allow(
    clippy::disallowed_methods,
    reason = "the event loop closes the window on its own thread: its flag is looked at every 10 ms, for at most 2 s"
)]
fn closed(open: &WebviewDriver) {
    open.close();
    let gone = std::time::Instant::now();
    while open.is_open() && gone.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

impl View for Page {
    fn conventions(&self) -> &str {
        CONVENTIONS
    }

    fn observe(&self) -> ViewResult<Observation> {
        let mut view = Observation::from(self.driver()?.observe()?);
        let controls = toolbar::controls(&crate::lock(&self.size));
        let mut bar = toolbar::outline(&controls);
        let (tabs, active) = self.driver()?.tabs();
        if tabs.len() > 1 {
            let names: Vec<String> = tabs
                .iter()
                .map(|tab| format!("{} \"{}\"", tab.id, tab.title))
                .collect();
            bar.push_str(&format!(
                "\n  - tabs: {} (this is {active})",
                names.join(", ")
            ));
        }
        view.outline = format!("{bar}\n{}", view.outline);
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
}
