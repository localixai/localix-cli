//! The worker's side of the native window: each [`Driver`] call becomes a probe call on the page.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tao::event_loop::EventLoopProxy;

use super::host::{Cmd, Loads, Shared, WINDOW_CLOSED};
use super::script::script_forms;
use super::{Action, BrowserError, BrowserResult, Clip, Driver, Observation, PageText, Shot};

const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// A script may await the page (a fetch, a timer); it gets this long.
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(20);
/// After a script's page started loading another document, how long its answer may still come:
/// a script that clicked Next and then awaited would otherwise hold the run for SCRIPT_TIMEOUT.
const SCRIPT_LEFT: Duration = Duration::from_millis(300);
/// What a script returns when its document went away before it answered.
const SCRIPT_LEFT_PAGE: &str = "(no value: the page loaded a new document while the script ran)";
const SETTLE_TIMEOUT: Duration = Duration::from_millis(2500);
/// How long a page that is replacing itself may take before the probe answers again.
const PROBE_READY: Duration = Duration::from_secs(5);
const LOAD_TIMEOUT: Duration = Duration::from_secs(20);
/// Once its DOM is parsed, how long a document gets to finish loading before it is used anyway.
const PARSED_GRACE: Duration = Duration::from_secs(1);
/// After an action, how long a navigation it caused has to show up.
const NAVIGATION_GRACE: Duration = Duration::from_millis(25);
/// After an action that requested a navigation the page did not report (a subframe's), how long
/// its load has to start.
const NAVIGATION_START: Duration = Duration::from_millis(300);
/// A settle that got no answer: its document most likely went away; a new load should follow.
const SETTLE_LOST: Duration = Duration::from_secs(5);
/// A long page may render off screen before WebKit draws its picture.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Clone)]
pub struct WebviewDriver {
    proxy: EventLoopProxy<Cmd>,
    shared: Arc<Shared>,
}

impl WebviewDriver {
    pub(super) fn new(proxy: EventLoopProxy<Cmd>, shared: Arc<Shared>) -> Self {
        Self { proxy, shared }
    }

    /// Whether the window is open: the person may have closed it.
    pub fn is_open(&self) -> bool {
        self.shared.open.load(Ordering::SeqCst)
    }

    fn open_or_closed(&self) -> BrowserResult<()> {
        match self.is_open() {
            true => Ok(()),
            false => Err(closed()),
        }
    }

    /// One probe call, answered over IPC or refused while the page has no probe.
    fn call(&self, name: &'static str, args: Value, timeout: Duration) -> BrowserResult<Value> {
        self.open_or_closed()?;
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        self.shared.pending.lock().unwrap().insert(id, sender);
        let sent = self.proxy.send_event(Cmd::Call {
            id,
            name,
            args: args.to_string(),
        });
        if sent.is_err() {
            self.shared.pending.lock().unwrap().remove(&id);
            return Err(BrowserError::Unavailable(
                "the browser window is gone".into(),
            ));
        }
        let reply = receiver.recv_timeout(timeout);
        self.shared.pending.lock().unwrap().remove(&id);
        match reply {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(classify(&error)),
            Err(_) => Err(BrowserError::Unavailable(format!(
                "the page did not answer {name}"
            ))),
        }
    }

    /// Retry `name` while the page is between documents (no probe yet, or no body).
    fn call_ready(&self, name: &'static str, args: Value) -> BrowserResult<Value> {
        let deadline = Instant::now() + PROBE_READY;
        loop {
            match self.call(name, args.clone(), CALL_TIMEOUT) {
                Ok(Value::Null) | Err(BrowserError::Unavailable(_))
                    if Instant::now() < deadline && self.is_open() =>
                {
                    std::thread::sleep(Duration::from_millis(30));
                }
                Ok(Value::Null) => {
                    return Err(BrowserError::Unavailable("the page has no document".into()));
                }
                other => return other,
            }
        }
    }

    fn loads(&self) -> Loads {
        *self.shared.loads.lock().unwrap()
    }

    /// Wait for a load newer than `before` to start, up to `grace`.
    fn load_started_since(&self, before: Loads, grace: Duration) -> bool {
        let guard = self.shared.loads.lock().unwrap();
        let (loads, _) = self
            .shared
            .loaded
            .wait_timeout_while(guard, grace, |loads| loads.started == before.started)
            .unwrap();
        loads.started != before.started
    }
}

impl WebviewDriver {
    fn run_script(&self, form: &str) -> BrowserResult<Value> {
        self.open_or_closed()?;
        let before = self.loads();
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        self.shared.pending.lock().unwrap().insert(id, sender);
        let script = form.replace("__ID__", &id.to_string());
        if self.proxy.send_event(Cmd::Eval { id, script }).is_err() {
            self.shared.pending.lock().unwrap().remove(&id);
            return Err(BrowserError::Unavailable(
                "the browser window is gone".into(),
            ));
        }
        let deadline = Instant::now() + SCRIPT_TIMEOUT;
        let mut left: Option<Instant> = None;
        let reply = loop {
            let now = Instant::now();
            let until = left.map_or(deadline, |at| (at + SCRIPT_LEFT).min(deadline));
            if now >= until {
                break Err(mpsc::RecvTimeoutError::Timeout);
            }
            match receiver.recv_timeout((until - now).min(Duration::from_millis(50))) {
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                other => break other,
            }
            if left.is_none() && self.loads().started != before.started {
                left = Some(Instant::now());
            }
        };
        self.shared.pending.lock().unwrap().remove(&id);
        match reply {
            Ok(Ok(value)) => Ok(value),
            Err(_) if left.is_some() => Ok(json!({"result": SCRIPT_LEFT_PAGE})),
            Ok(Err(error)) if error.starts_with("script_not_started") => {
                Err(BrowserError::Unavailable(error))
            }
            Ok(Err(error)) if error == WINDOW_CLOSED => Err(closed()),
            Ok(Err(error)) => Err(BrowserError::Refused(error)),
            Err(_) => Err(BrowserError::Unavailable(
                "the script did not finish in time".into(),
            )),
        }
    }

    /// After an action: if it replaced the document, wait for the new one to load. The page knows
    /// first (`navigating`; a settle that died with its document says the same); a navigation the
    /// native side saw requested but the page did not (a subframe) gets a short look only.
    fn follow_navigation(&self, before: Loads, settled: &BrowserResult<Value>) {
        let requested = self.loads().requested != before.requested;
        let flag = |key| {
            settled
                .as_ref()
                .is_ok_and(|value| value.get(key).and_then(Value::as_bool) == Some(true))
        };
        let grace = match settled {
            _ if flag("navigating") => LOAD_TIMEOUT,
            Err(_) => SETTLE_LOST,
            // Frames request navigations all the time (ad syncs); with the Navigation API the page
            // has reported its own, so theirs are not waited for.
            _ if requested && !flag("watched") => NAVIGATION_START,
            _ => NAVIGATION_GRACE,
        };

        if self.load_started_since(before, grace) {
            let _ = self.wait_loaded(LOAD_TIMEOUT);
        }
    }
}

/// Why nothing can be done on the page: the person closed the window.
fn closed() -> BrowserError {
    BrowserError::Unavailable("the browser window was closed".into())
}

fn classify(error: &str) -> BrowserError {
    if error == WINDOW_CLOSED {
        closed()
    } else if error.starts_with("probe_not_ready") {
        BrowserError::Unavailable(error.to_string())
    } else if error.starts_with("stale") {
        BrowserError::Stale(error.to_string())
    } else {
        BrowserError::Refused(error.to_string())
    }
}

fn action_args(epoch: &str, action: &Action) -> Value {
    match action {
        Action::Click { reference } => json!({"op": "click", "ref": reference, "epoch": epoch}),
        Action::Type { reference, text } => {
            json!({"op": "type", "ref": reference, "value": text, "epoch": epoch})
        }
        Action::Select { reference, value } => {
            json!({"op": "select", "ref": reference, "value": value, "epoch": epoch})
        }
        Action::PressEnter { reference } => {
            json!({"op": "press_enter", "ref": reference, "epoch": epoch})
        }
        Action::ScrollDown => json!({"op": "scroll_down", "epoch": epoch}),
        Action::ScrollUp => json!({"op": "scroll_up", "epoch": epoch}),
    }
}

impl WebviewDriver {
    /// Close the window; calls waiting on it fail, and the next `Page::open` opens a new one.
    pub fn close(&self) {
        let _ = self.proxy.send_event(Cmd::Close);
    }
}

impl Driver for WebviewDriver {
    fn navigate(&self, url: &str) -> BrowserResult<()> {
        self.open_or_closed()?;
        let before = self.loads();
        self.proxy
            .send_event(Cmd::Navigate(url.to_string()))
            .map_err(|_| BrowserError::Unavailable("the browser window is gone".into()))?;
        self.load_started_since(before, PROBE_READY);
        self.wait_loaded(LOAD_TIMEOUT)
    }

    /// Until the document is loaded, or parsed (DOMContentLoaded, as its probe says) for
    /// PARSED_GRACE. The `load` event also waits for every image, video and ad frame, which on a
    /// busy site comes many seconds later or never; the page stays `busy` in its snapshot until then.
    fn wait_loaded(&self, timeout: Duration) -> BrowserResult<()> {
        let deadline = Instant::now() + timeout;
        let mut parsed: Option<Instant> = None;
        loop {
            self.open_or_closed()?;
            let loads = self.loads();
            if loads.started > 0 && loads.finished >= loads.started {
                return Ok(());
            }
            if parsed.is_some_and(|at| at.elapsed() >= PARSED_GRACE) {
                return Ok(());
            }
            // Load events come from the committed document only: a probe answering now is the
            // new document's.
            if parsed.is_none()
                && loads.started > 0
                && let Ok(Value::String(state)) =
                    self.call("ready", Value::Null, Duration::from_millis(500))
                && state != "loading"
            {
                parsed = Some(Instant::now());
            }
            if Instant::now() >= deadline {
                return Err(BrowserError::Unavailable(
                    "the page is still loading".into(),
                ));
            }
            let guard = self.shared.loads.lock().unwrap();
            let _ = self
                .shared
                .loaded
                .wait_timeout(guard, Duration::from_millis(50))
                .unwrap();
        }
    }

    fn observe(&self) -> BrowserResult<Observation> {
        let value = self.call_ready("snapshot", Value::Null)?;
        serde_json::from_value(value)
            .map_err(|error| BrowserError::Unavailable(format!("unreadable snapshot: {error}")))
    }

    fn act(&self, epoch: &str, action: &Action) -> BrowserResult<()> {
        let debug = std::env::var_os("BROWSER_DEBUG").is_some();
        let started = Instant::now();
        let before = self.loads();
        let args = action_args(epoch, action);
        self.call("act", args.clone(), CALL_TIMEOUT)?;
        let acted = started.elapsed();
        let settled = self.call("settle", args, SETTLE_TIMEOUT);
        let settle_time = started.elapsed() - acted;
        self.follow_navigation(before, &settled);
        if debug {
            eprintln!("{action:?}");
            eprintln!(
                "act {acted:?} settle {settle_time:?} ({settled:?}) total {:?}",
                started.elapsed()
            );
        }
        Ok(())
    }

    fn resize(&self, width: f64, height: f64) -> BrowserResult<()> {
        self.proxy
            .send_event(Cmd::Resize(width, height))
            .map_err(|_| BrowserError::Unavailable("the browser window is gone".into()))?;
        // The page reflows on its own resize event; give it the usual settle.
        let _ = self.call("settle", json!({}), SETTLE_TIMEOUT);
        Ok(())
    }

    fn press(&self, keys: &[String], delay_ms: u64) -> BrowserResult<()> {
        let before = self.loads();
        let budget = Duration::from_millis(delay_ms * keys.len() as u64) + CALL_TIMEOUT;
        self.call("keys", json!({"keys": keys, "delay": delay_ms}), budget)?;
        let settled = self.call("settle", json!({}), SETTLE_TIMEOUT);
        self.follow_navigation(before, &settled);
        Ok(())
    }

    fn evaluate(&self, source: &str) -> BrowserResult<String> {
        let before = self.loads();
        let mut value = Err(BrowserError::Refused("the script does not parse".into()));
        for form in script_forms(source) {
            value = self.run_script(&form);
            if !matches!(&value, Err(BrowserError::Unavailable(e)) if e.starts_with("script_not_started"))
            {
                break;
            }
        }
        let value = value.map_err(|error| match error {
            BrowserError::Unavailable(e) if e.starts_with("script_not_started") => {
                BrowserError::Refused("the script does not parse, or the page has no probe".into())
            }
            other => other,
        })?;
        let settled = self.call("settle", json!({}), SETTLE_TIMEOUT);
        self.follow_navigation(before, &settled);
        Ok(value
            .get("result")
            .and_then(Value::as_str)
            .unwrap_or("undefined")
            .to_string())
    }

    fn read(&self, offset: usize, limit: usize) -> BrowserResult<PageText> {
        let value = self.call_ready("read", json!({"offset": offset, "limit": limit}))?;
        serde_json::from_value(value)
            .map_err(|error| BrowserError::Unavailable(format!("unreadable text: {error}")))
    }

    fn screenshot(&self, selector: Option<&str>) -> BrowserResult<Shot> {
        self.open_or_closed()?;
        let clip = match selector {
            Some(selector) => {
                let at = self.call_ready("locate", json!({"selector": selector}))?;
                let side = |key: &str| at.get(key).and_then(Value::as_f64).unwrap_or(0.0);
                Some(Clip {
                    x: side("x"),
                    y: side("y"),
                    width: side("width"),
                    height: side("height"),
                })
            }
            None => None,
        };
        let (reply, shot) = mpsc::channel();
        self.proxy
            .send_event(Cmd::Capture { clip, reply })
            .map_err(|_| BrowserError::Unavailable("the browser window is gone".into()))?;
        match shot.recv_timeout(CAPTURE_TIMEOUT) {
            Ok(Ok(shot)) => Ok(shot),
            Ok(Err(why)) => Err(BrowserError::Refused(format!("no screenshot: {why}"))),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(BrowserError::Unavailable(
                "the screenshot did not come in time".into(),
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(closed()),
        }
    }
}
