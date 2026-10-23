use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use super::*;
use crate::browser::host::Cmd;

#[test]
fn a_panels_window_routes_to_that_panel() {
    assert_eq!(route(7, Some(1), &[5, 7, 9]), Route::Panel(1));
    assert_eq!(route(5, None, &[5]), Route::Panel(0));
}

#[test]
fn the_browsers_window_routes_to_the_browser() {
    assert_eq!(route(1, Some(1), &[5, 7]), Route::Browser);
}

#[test]
fn an_unknown_window_routes_nowhere() {
    assert_eq!(route(3, Some(1), &[5, 7]), Route::Other);
    // A closed browser leaves its id to no one: a late event for it is not the browser's.
    assert_eq!(route(1, None, &[]), Route::Other);
}

/// A panel whose commands are recorded, not sent: what reached the loop, as text.
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

#[test]
fn panels_open_at_the_screens_right_edge_cascaded() {
    let screen = (0.0, 25.0, 1728.0, 1092.0);
    let first = place(screen, 1);
    assert_eq!(first, (1728.0 - WIDTH - 16.0 - 28.0, 25.0 + 60.0 + 28.0));
    assert_eq!(
        place(screen, 5),
        first,
        "every fourth one where the first was"
    );
    assert_eq!(place(screen, 2).0, first.0 - 28.0);
    // A screen narrower than the window: its left edge.
    assert_eq!(place((-800.0, 0.0, 600.0, 400.0), 4).0, -800.0);
}
