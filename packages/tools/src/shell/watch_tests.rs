//! Sessions shown in windows, through a watch that records what it opens.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use super::super::session::{Sessions, Until, Wait};
use super::*;
use crate::testing::Scratch;

/// The windows a watch opened: their titles, their frames, and how many were closed.
#[derive(Clone, Default)]
struct Windows {
    titles: Arc<Mutex<Vec<String>>>,
    frames: Arc<Mutex<Vec<String>>>,
    /// The person closed them: a paint says so.
    closed_by_person: Arc<AtomicBool>,
    drops: Arc<AtomicUsize>,
}

struct Dropped(Arc<AtomicUsize>);

impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl Windows {
    fn watch(&self) -> Watch {
        let windows = self.clone();
        Arc::new(move |_id: u32, title: &str| {
            windows.titles.lock().unwrap().push(title.to_string());
            let (frames, closed) = (
                Arc::clone(&windows.frames),
                Arc::clone(&windows.closed_by_person),
            );
            let dropped = Dropped(Arc::clone(&windows.drops));
            Some(Box::new(move |frame: String| {
                let _ = &dropped;
                frames.lock().unwrap().push(frame);
                !closed.load(Ordering::SeqCst)
            }) as Painter)
        })
    }

    fn opened(&self) -> Vec<String> {
        self.titles.lock().unwrap().clone()
    }

    fn drops(&self) -> usize {
        self.drops.load(Ordering::SeqCst)
    }

    fn last(&self) -> String {
        self.frames
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }

    /// Whether a frame came that contains `text`, within two seconds.
    fn shows(&self, text: &str) -> bool {
        eventually(Duration::from_secs(2), || self.last().contains(text))
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "a test looks again until it is so, for a bounded time: there is no event to wait on"
)]
fn eventually(within: Duration, done: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + within;
    while !done() {
        if Instant::now() > deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(20));
    }
    true
}

fn never() -> bool {
    false
}

fn quick() -> Wait<'static> {
    Wait {
        needle: None,
        yield_ms: Some(200),
    }
}

fn start(sessions: &Sessions, command: &str, show: Option<bool>) -> u32 {
    let dir = Scratch::new();
    let turn = sessions.start(
        command,
        dir.path(),
        Default::default(),
        quick(),
        show,
        &never,
    );
    turn.unwrap().id
}

#[test]
fn a_command_that_ends_at_once_opens_no_window() {
    let windows = Windows::default();
    let sessions = Sessions::new(windows.watch(), true);
    let dir = Scratch::new();
    let turn = sessions.start(
        "printf hi",
        dir.path(),
        Default::default(),
        Wait::default(),
        None,
        &never,
    );
    assert!(matches!(turn.unwrap().until, Until::Exited(_)));
    assert!(windows.opened().is_empty());
}

#[test]
fn a_kept_session_opens_one_window_that_follows_it() {
    let windows = Windows::default();
    let sessions = Sessions::new(windows.watch(), true);
    let id = start(&sessions, "cat", None);
    assert_eq!(windows.opened(), ["session 1 · cat"]);
    sessions
        .send(id, b"abc\n", &[], quick(), None, &never)
        .unwrap();
    assert!(windows.shows("abc"), "{}", windows.last());
    sessions
        .send(id, b"", &[], quick(), Some(true), &never)
        .unwrap();
    assert_eq!(
        windows.opened().len(),
        1,
        "an open window is not opened twice"
    );
}

/// The painter is the app's: it is called with the session's lock let go (ARCHITECTURE.md R6),
/// so while it paints, the session's screen is read at once on another thread.
#[test]
fn a_window_is_painted_with_the_session_free() {
    let reach: Arc<OnceLock<Weak<Sessions>>> = Arc::default();
    let (blocked, painted) = (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicUsize::new(0)),
    );
    let (seen, counted) = (Arc::clone(&blocked), Arc::clone(&painted));
    let sessions_of = Arc::clone(&reach);
    let watch: Watch = Arc::new(move |_id: u32, _title: &str| {
        let (reach, blocked, painted) = (
            Arc::clone(&sessions_of),
            Arc::clone(&seen),
            Arc::clone(&counted),
        );
        Some(Box::new(move |_frame: String| {
            if let Some(sessions) = reach.get().and_then(Weak::upgrade) {
                let (read, done) = mpsc::sync_channel(1);
                let reader = thread::Builder::new().name("reads".into());
                let _ = reader.spawn(move || {
                    for (id, _) in sessions.listed() {
                        let _ = sessions.screen(id);
                    }
                    let _ = read.send(());
                });
                if done.recv_timeout(Duration::from_secs(2)).is_err() {
                    blocked.store(true, Ordering::SeqCst);
                }
            }
            painted.fetch_add(1, Ordering::SeqCst);
            true
        }) as Painter)
    });
    let sessions = Arc::new(Sessions::new(watch, true));
    reach.set(Arc::downgrade(&sessions)).unwrap();
    start(&sessions, "sleep 0.3; printf one; sleep 30", None);
    let painted_twice = || painted.load(Ordering::SeqCst) >= 2;
    assert!(eventually(Duration::from_secs(10), painted_twice));
    assert!(!blocked.load(Ordering::SeqCst), "a paint held the lock");
    sessions.close_all();
}

#[test]
fn every_end_by_a_call_closes_the_window_at_once() {
    type End = fn(&Sessions, u32);
    let ends: [(&str, End); 3] = [
        ("close", |sessions, id| {
            let _ = sessions.close(id);
        }),
        ("close_all", |sessions, _| {
            let _ = sessions.close_all();
        }),
        ("drop", |_, _| {}),
    ];
    for (name, end) in ends {
        let windows = Windows::default();
        let sessions = Sessions::new(windows.watch(), true);
        let id = start(&sessions, "sleep 30", None);
        end(&sessions, id);
        if name == "drop" {
            drop(sessions);
        }
        assert_eq!(windows.drops(), 1, "{name}");
    }
}

#[test]
fn a_stop_leaves_the_window_open() {
    let windows = Windows::default();
    let sessions = Sessions::new(windows.watch(), true);
    let id = start(&sessions, "sleep 30", None);
    let _ = sessions.send(id, b"", &[], quick(), None, &|| true);
    assert_eq!(windows.drops(), 0, "the program runs on, and shows");
}

#[test]
fn a_program_that_ends_alone_says_so_then_its_window_closes() {
    let windows = Windows::default();
    let sessions = Sessions::new(windows.watch(), true);
    start(&sessions, "sleep 0.5; exit 3", None);
    // No call waits on it: the reader sees the end.
    assert!(windows.shows("exited 3"), "{}", windows.last());
    let ended = Instant::now();
    assert!(eventually(Duration::from_secs(5), || windows.drops() == 1));
    let kept = ended.elapsed();
    assert!(
        kept >= LINGER.saturating_sub(Duration::from_millis(300)),
        "{kept:?}"
    );
}

#[test]
fn a_program_that_ends_in_a_call_says_so_too() {
    let windows = Windows::default();
    let sessions = Sessions::new(windows.watch(), true);
    let id = start(&sessions, "read line; exit 0", None);
    let turn = sessions.send(id, b"x\n", &[], quick(), None, &never);
    assert_eq!(
        turn.unwrap().until,
        Until::Exited(super::super::Ending::Exit(0))
    );
    assert!(windows.shows("exited 0"), "{}", windows.last());
    assert_eq!(windows.drops(), 0, "it stays a moment");
    assert!(eventually(Duration::from_secs(5), || windows.drops() == 1));
}

#[test]
fn show_opens_and_closes_and_a_window_closed_by_the_person_stays_closed() {
    let windows = Windows::default();
    let sessions = Sessions::new(windows.watch(), true);
    let id = start(&sessions, "cat", Some(false));
    assert!(windows.opened().is_empty(), "hidden at start");
    let send = |show| sessions.send(id, b"", &[], quick(), show, &never).unwrap();
    send(Some(true));
    assert_eq!(windows.opened().len(), 1);
    send(Some(false));
    assert_eq!(windows.drops(), 1);
    send(Some(true));
    assert_eq!(windows.opened().len(), 2, "shown again");
    // The person closes it: the next paint says so, and it is not opened again unasked.
    windows.closed_by_person.store(true, Ordering::SeqCst);
    sessions
        .send(id, b"more\n", &[], quick(), None, &never)
        .unwrap();
    assert!(eventually(Duration::from_secs(2), || windows.drops() == 2));
    send(None);
    assert_eq!(windows.opened().len(), 2);
    windows.closed_by_person.store(false, Ordering::SeqCst);
    send(Some(true));
    assert_eq!(windows.opened().len(), 3, "asked for again");
}

#[test]
fn hidden_sessions_open_windows_only_when_asked() {
    let windows = Windows::default();
    let sessions = Sessions::new(windows.watch(), false);
    let id = start(&sessions, "cat", None);
    assert!(windows.opened().is_empty());
    sessions
        .send(id, b"", &[], quick(), Some(true), &never)
        .unwrap();
    assert_eq!(windows.opened().len(), 1);
}

#[test]
fn without_a_watch_show_does_nothing() {
    let sessions = Sessions::default();
    let id = start(&sessions, "cat", Some(true));
    let turn = sessions.send(id, b"hi\n", &[], quick(), Some(true), &never);
    assert!(turn.unwrap().output.unwrap_or_default().contains("hi"));
}

#[test]
fn the_last_of_a_burst_is_painted_with_no_more_output() {
    let windows = Windows::default();
    let sessions = Sessions::new(windows.watch(), true);
    start(
        &sessions,
        "sleep 0.4; printf one; sleep 0.01; printf two; sleep 30",
        None,
    );
    // The burst comes faster than a frame: the reader wakes to paint its end.
    assert!(windows.shows("onetwo"), "{}", windows.last());
}
