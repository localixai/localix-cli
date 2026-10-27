//! The browser window and the page it shows, as a `View`: the window opens on the first address
//! it is sent to, and later addresses load in it. The browser's own toolbar (Back, Reload) is part of the view, so the hands can use it like any control; with several tabs open,
//! it also says which tab the view is, but the hands do not switch tabs: the agent does.

use std::fmt::Write as _;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use contracts::view::{Action, Observation, View, ViewError, ViewResult};

use super::{Browser, Driver, Viewport, WebviewDriver, Window, toolbar};

/// How web pages behave where it is not obvious: forms, suggestions, date pickers, results.
const CONVENTIONS: &str = "A form sends only once its required fields are filled and its Search, \
Submit or Done button is pressed. A typed search query usually needs its matching suggestion \
picked from the list that appears. A date picker takes the date and then its confirmation (Done, \
Apply or OK). A result opens by its link; a matching link is not yet the result. In a quiz or a \
form, the answer or option is picked before Next, Continue or Submit.";

/// The window as a view. Its lock is held only to look at the window or change what is held of
/// it, never across a load or a reply of the window's loop (ARCHITECTURE.md R6): a page that
/// loads still answers other threads. Calls in one tab are one at a time by the caller's choice
/// (the app's lock per tab).
pub struct Page {
    browser: Browser,
    /// Hidden unless a call asks to show it: the default the app chose for this run.
    headless: bool,
    held: Mutex<Held>,
    /// Rung when an opening ends, for the calls that wait to open the window.
    opened: Condvar,
}

/// The window as a page holds it.
struct Held {
    driver: Option<WebviewDriver>,
    /// Whether the open window is hidden; it is opened again when a call wants the other.
    hidden: bool,
    /// A call is opening the window: the others wait for it rather than open a second one.
    opening: bool,
}

/// What a call that wants the window found, once no other call was opening it.
enum Claim {
    /// The window, open as the call wants it.
    Open(WebviewDriver),
    /// None open as wanted: this call opens it, the one open the other way closed first.
    Make(Option<WebviewDriver>),
}

impl Page {
    pub fn new(browser: Browser, headless: bool) -> Self {
        Self {
            browser,
            headless,
            held: Mutex::new(Held {
                driver: None,
                hidden: headless,
                opening: false,
            }),
            opened: Condvar::new(),
        }
    }

    /// Load `url`, opening the window on it the first time (and again after the person closed
    /// it); `title` names the window. `show` overrides the run's default for this call: a window
    /// open the other way is closed and opened again, on `url`.
    pub fn open(&self, url: &str, title: &str, show: Option<bool>) -> ViewResult<()> {
        let hidden = show.map_or(self.headless, |show| !show);
        match self.claim(|now| now == hidden) {
            Claim::Open(open) => open.navigate(url),
            Claim::Make(stale) => {
                let (driver, tab) = self.make(url, title, hidden, stale)?;
                // A slow page is used as it is: it says it is busy until it settles.
                driver.on(tab).wait_new();
                Ok(())
            }
        }
    }

    /// Open `url` in a tab of its own and say the tab's id, its document not waited for (a driver
    /// pinned to the tab waits: `wait_new`): a new tab of the window open, else the window opened
    /// on it.
    pub fn start(&self, url: &str) -> ViewResult<u32> {
        match self.claim(|_| true) {
            Claim::Open(open) => open.add_tab(url),
            Claim::Make(stale) => self.make(url, url, self.headless, stale).map(|made| made.1),
        }
    }

    /// Once no call is opening the window: the window if it is open as `wanted` says of its
    /// being hidden, else this call's claim to open it. The look and the claim are one step under
    /// the lock (R7), so two calls never open two windows; the opening itself is done with the
    /// lock let go, while the calls that want the window wait on `opened` (R9).
    fn claim(&self, wanted: impl Fn(bool) -> bool) -> Claim {
        let held = self.held();
        let mut held = (self.opened)
            .wait_while(held, |held| held.opening)
            .unwrap_or_else(PoisonError::into_inner);
        let open = held.driver.clone().filter(WebviewDriver::is_open);
        match open {
            Some(open) if wanted(held.hidden) => Claim::Open(open),
            stale => {
                held.opening = true;
                Claim::Make(stale)
            }
        }
    }

    /// The window made for `url` by the call that claimed it (`stale` closed first): a tab of its
    /// own in the window open for someone else (another client of the daemon, whose tabs stay as
    /// they are), else the window opened on it. Its driver is held at once and the claim ends,
    /// however this does; the tab's document is not waited for. The driver and the tab's id.
    fn make(
        &self,
        url: &str,
        title: &str,
        hidden: bool,
        stale: Option<WebviewDriver>,
    ) -> ViewResult<(WebviewDriver, u32)> {
        let _claim = Claimed(self);
        if let Some(stale) = stale {
            closed(&stale);
        }
        let attach = |attached: WebviewDriver| {
            let tab = (attached.add_tab(url))
                .map_err(|error| ViewError::Unavailable(error.to_string()))?;
            self.held().driver = Some(attached.clone());
            Ok((attached, tab))
        };
        if let Some(attached) = self.browser.attach() {
            return attach(attached);
        }
        self.held().hidden = hidden;
        let opened = match self.browser.open(Window {
            url: url.to_string(),
            title: title.to_string(),
            headless: hidden,
            viewport: Viewport::DESKTOP,
        }) {
            Ok(opened) => opened,
            // Another page opened it between the look and this: a tab of its own in it.
            Err(refused) => return self.browser.attach().map_or(Err(refused), attach),
        };
        let (_, tab) = opened.tabs();
        self.held().driver = Some(opened.clone());
        Ok((opened, tab))
    }

    pub fn is_open(&self) -> bool {
        (self.held().driver.as_ref()).is_some_and(WebviewDriver::is_open)
    }

    /// The open window's driver, for what only a browser has (scripts, keys, the whole text).
    pub fn driver(&self) -> ViewResult<WebviewDriver> {
        let driver = self.held().driver.clone();
        driver
            .filter(WebviewDriver::is_open)
            .ok_or_else(|| ViewError::Unavailable("the browser window is not open".into()))
    }

    /// The same window acting in tab `id`, whatever is shown: a page of its own to work in
    /// beside the others.
    #[must_use]
    pub fn pinned(&self, id: u32) -> Page {
        let (driver, hidden) = {
            let held = self.held();
            (held.driver.as_ref().map(|d| d.on(id)), held.hidden)
        };
        Page {
            browser: self.browser.clone(),
            headless: self.headless,
            held: Mutex::new(Held {
                driver,
                hidden,
                opening: false,
            }),
            opened: Condvar::new(),
        }
    }

    fn held(&self) -> MutexGuard<'_, Held> {
        crate::lock(&self.held)
    }

    /// A toolbar control: back or reload.
    fn toolbar(&self, action: &Action) -> ViewResult<()> {
        let driver = self.driver()?;
        match action {
            Action::Click { reference } if reference == toolbar::BACK => {
                driver.evaluate("history.back()").map(|_| ())
            }
            Action::Click { reference } if reference == toolbar::RELOAD => {
                driver.evaluate("location.reload()").map(|_| ())
            }
            Action::Click { .. }
            | Action::Select { .. }
            | Action::Type { .. }
            | Action::PressEnter { .. }
            | Action::ScrollDown
            | Action::ScrollUp => Err(ViewError::Refused("a toolbar control is clicked".into())),
        }
    }

    /// Reframe the page to a size (`desktop`, `tablet`, `mobile`).
    pub fn resize(&self, size: &str) -> ViewResult<()> {
        let (width, height) = toolbar::size(size)
            .ok_or_else(|| ViewError::Refused(format!("no page size {size}")))?;
        self.driver()?.resize(width, height)
    }
}

/// A call's claim to open the window, ended however its opening ends: the calls waiting for it
/// are woken.
struct Claimed<'a>(&'a Page);

impl Drop for Claimed<'_> {
    fn drop(&mut self) {
        self.0.held().opening = false;
        self.0.opened.notify_all();
    }
}

/// Close `open` and wait, briefly, until it is.
#[expect(
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
        let controls = toolbar::controls();
        let mut bar = toolbar::outline(&controls);
        let (tabs, active) = self.driver()?.tabs();
        if tabs.len() > 1 {
            let names: Vec<String> = tabs
                .iter()
                .map(|tab| format!("{} \"{}\"", tab.id, tab.title))
                .collect();
            let _ = write!(bar, "\n  - tabs: {} (this is {active})", names.join(", "));
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
            Action::PressEnter { .. } | Action::ScrollDown | Action::ScrollUp => None,
        };
        match reference {
            Some(reference) if toolbar::owns(reference) => self.toolbar(action),
            _ => self.driver()?.act(epoch, action),
        }
    }
}
