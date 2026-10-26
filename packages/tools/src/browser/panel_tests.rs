use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use super::*;
use crate::browser::host::Cmd;

/// A panel whose commands are recorded, not sent: what reached the loop, as text.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn recorded(accepted: bool) -> (Panel, Arc<Mutex<Vec<String>>>) {
    let sent = Arc::new(Mutex::new(vec![]));
    let log = Arc::clone(&sent);
    let send = move |command: Cmd| {
        let line = match command {
            Cmd::Paint(id) => format!("paint {id}"),
            Cmd::Unpanel(id) => format!("unpanel {id}"),
            _ => "other".into(),
        };
        log.lock().unwrap().push(line);
        accepted
    };
    (Panel::new(4, Box::new(send)), sent)
}

#[test]
fn paints_before_the_loop_runs_queue_one_command_and_the_loop_takes_the_newest() {
    let (panel, sent) = recorded(true);
    for frame in ["one", "two", "three"] {
        assert!(panel.paint(frame.into()));
    }
    assert_eq!(*sent.lock().unwrap(), ["paint 4"]);
    assert_eq!(panel.slot.latest().as_deref(), Some("three"));
    // Taken: the next paint queues again, and the frame stays for a page that loads late.
    assert!(panel.paint("four".into()));
    assert_eq!(*sent.lock().unwrap(), ["paint 4", "paint 4"]);
    assert_eq!(panel.slot.latest().as_deref(), Some("four"));
    assert_eq!(panel.slot.latest().as_deref(), Some("four"));
}

#[test]
fn dropping_a_panel_closes_its_window_once() {
    let (panel, sent) = recorded(true);
    drop(panel);
    assert_eq!(*sent.lock().unwrap(), ["unpanel 4"]);
}

#[test]
fn a_paint_says_whether_the_window_is_still_open() {
    let (panel, _) = recorded(true);
    assert!(panel.paint("a".into()));
    panel.slot.alive.store(false, Ordering::SeqCst);
    assert!(!panel.paint("b".into()), "the person closed it");
    let (gone, _) = recorded(false);
    assert!(!gone.paint("a".into()), "the loop is gone");
}
