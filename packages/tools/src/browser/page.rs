//! The browser window and the page it shows, as a `View`: the window opens on the first address
//! it is sent to, and later addresses load in it. The browser's own toolbar (Back, Reload, page
//! size) is part of the view, so the hands can use it like any control; with several tabs open,
//! it also says which tab the view is, but the hands do not switch tabs: the agent does.

use std::sync::Mutex;
use std::time::Duration;

use contracts::view::{Action, Observation, View, ViewError, ViewResult};

use super::{Browser, Driver, PageText, Tab, Viewport, WebviewDriver, Window, toolbar};

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
    /// The tabs as the agent was last told of them: they are told again only once they change.
    told: Mutex<String>,
}

impl Page {
    pub fn new(browser: Browser, headless: bool) -> Self {
        Self {
            browser,
            headless,
            hidden: Mutex::new(headless),
            size: Mutex::new("desktop".into()),
            driver: Mutex::new(None),
            told: Mutex::new(String::new()),
        }
    }

    /// Load `url`, opening the window on it the first time (and again after the person closed
    /// it); `title` names the window. `show` overrides the run's default for this call: a window
    /// open the other way is closed and opened again, on `url`.
    pub fn open(&self, url: &str, title: &str, show: Option<bool>) -> ViewResult<()> {
        let mut driver = self.driver.lock().unwrap();
        let hidden = show.map_or(self.headless, |show| !show);
        let mut current = self.hidden.lock().unwrap();
        if let Some(open) = driver.as_ref().filter(|driver| driver.is_open()) {
            if *current == hidden {
                return open.navigate(url);
            }
            closed(open);
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
        let driver = self.driver.lock().unwrap();
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
        *self.hidden.lock().unwrap()
    }

    pub fn is_open(&self) -> bool {
        self.driver
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(WebviewDriver::is_open)
    }

    /// The open window's driver, for what only a browser has (scripts, keys, the whole text).
    pub fn driver(&self) -> ViewResult<WebviewDriver> {
        self.driver
            .lock()
            .unwrap()
            .clone()
            .filter(WebviewDriver::is_open)
            .ok_or_else(|| ViewError::Unavailable("the browser window is not open".into()))
    }

    /// Open `url` in a new tab and act on it from now on.
    pub fn new_tab(&self, url: &str) -> ViewResult<u32> {
        self.driver()?.new_tab(url)
    }

    /// Act on tab `id` from now on.
    pub fn switch_tab(&self, id: u32) -> ViewResult<()> {
        self.driver()?.switch_tab(id)
    }

    /// Close tab `id`; whether tabs are left (with none, the window closed).
    pub fn close_tab(&self, id: u32) -> ViewResult<bool> {
        self.driver()?.close_tab(id)
    }

    /// The tabs, when they changed since the agent was last told: one line each, the shown one
    /// marked. Nothing while a single tab was and is open.
    pub fn tabs_news(&self) -> Option<String> {
        let (tabs, active) = self.driver().map(|d| d.tabs()).unwrap_or_default();
        // Told again when tabs open, close, move or the shown one changes, not when a title
        // ticks.
        let key = match tabs.len() {
            0 | 1 => String::new(),
            _ => tabs
                .iter()
                .map(|tab| format!("{} {}", tab.id, tab.url))
                .chain([active.to_string()])
                .collect::<Vec<_>>()
                .join("\n"),
        };
        let mut told = self.told.lock().unwrap();
        if *told == key {
            return None;
        }
        *told = key.clone();
        Some(match key.is_empty() {
            true => "### Tabs\nOne tab is open.".to_string(),
            false => format!("### Tabs\n{}", listing(&tabs, active)),
        })
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

/// Close `open` and wait, briefly, until it is.
fn closed(open: &WebviewDriver) {
    open.close();
    let gone = std::time::Instant::now();
    while open.is_open() && gone.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The tabs, one line each: `2. Title — address (shown)`.
fn listing(tabs: &[Tab], active: u32) -> String {
    tabs.iter()
        .map(|tab| {
            let shown = if tab.id == active { " (shown)" } else { "" };
            format!("{}. {} — {}{shown}", tab.id, tab.title, tab.url)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl View for Page {
    fn conventions(&self) -> &str {
        CONVENTIONS
    }

    fn observe(&self) -> ViewResult<Observation> {
        let mut view = Observation::from(self.driver()?.observe()?);
        let controls = toolbar::controls(&self.size.lock().unwrap());
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
