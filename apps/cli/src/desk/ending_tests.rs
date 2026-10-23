use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::compose::{Models, Seat, Shared, lane_with};
use crate::desk::test_support::{Person, at_desk, at_desk_with, call, ended, state};
use crate::desk::{Work, run};
use crate::mode::Mode;
use crate::testing::{NoDecider, Scratch, gated, says, setup};
use crate::tui::Ui;
use crate::tui::app::{Action, App};
use crate::tui::views::AgentState;

#[test]
fn stopping_lane_b_leaves_lane_a_running() {
    let ((to_a, a), (to_b, b)) = (gated(), gated());
    at_desk(&[&a, &b], Some("a's task"), |person| {
        person.until("A asks its model", |_| a.turns() == 1);
        let lane_b = person.new_lane();
        person.enter("b's task");
        person.until("B asks its model", |_| b.turns() == 1);
        person.until("B works", |app| app.busy().is_some());
        let action = person
            .app
            .event(crate::tui::test_support::ctrl('c'), Instant::now());
        assert_eq!(action, Some(Action::Stop(lane_b.clone())));
        person.act(action, false);
        to_b.send(call("read", json!({"path": "."}))).unwrap();
        person.until("B ends", |app| ended(app, &lane_b));
        assert_eq!(
            state(&person.app, &lane_b),
            AgentState::Ended("interrupted".into())
        );
        let lane_a = person.first();
        assert_eq!(state(&person.app, &lane_a), AgentState::Working);
        to_a.send(says("A is done.")).unwrap();
        person.until("A ends", |app| ended(app, &lane_a));
        assert_eq!(
            state(&person.app, &lane_a),
            AgentState::Ended("achieved".into())
        );
    });
}

#[test]
fn a_panicking_lane_ends_the_other_lanes() {
    let (_to_a, a) = gated();
    let begun = Instant::now();
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let scratch = Scratch::new();
        let (shared, seat) = setup(&scratch, Mode::Bypass, true).split(None);
        let (events, inbox) = channel();
        let ui = Ui::new(events);
        let (requests, asked) = channel();
        let seat = Seat {
            ui: Some(ui.clone()),
            ..seat
        };
        let next = AtomicUsize::new(0);
        let start = |shared: &Shared, seat: Seat, work: Work| {
            assert_eq!(
                next.fetch_add(1, Ordering::SeqCst),
                0,
                "a bug in the second lane"
            );
            let models = Models {
                chat: &a,
                decider: &NoDecider,
                live: None,
            };
            lane_with(shared, seat, models, work)
        };
        std::thread::scope(|scope| {
            let desk = scope.spawn(|| run(&shared, &ui, asked, seat, None, &start, &|| {}));
            let mut person = Person {
                app: App::new(80, 24, vec![]),
                events: inbox,
                requests,
                over: false,
            };
            person.until("A waits", |app| app.lane().waiting.is_some());
            person.enter("/new");
            person.until_over();
            // The interface is gone: what A waits for never comes.
            drop(person);
            desk.join()
        })
    }));
    assert!(
        matches!(unwound, Ok(Err(_)) | Err(_)),
        "the panic reaches the desk's caller"
    );
    assert!(begun.elapsed() < Duration::from_secs(5));
}

#[test]
fn quit_waits_at_most_the_bound() {
    // A model that never answers: only the bound ends the wait.
    let (to_a, a) = gated();
    let sender = Mutex::new(Some(to_a));
    let quit_at = Mutex::new(None);
    let asked_at = Mutex::new(None);
    let quit = || {
        *quit_at.lock().unwrap() = Some(Instant::now());
        // What the process's end does to a request in flight: it is gone.
        sender.lock().unwrap().take();
    };
    at_desk_with(&[&a], Some("hang"), &quit, |person| {
        person.until("A asks its model", |_| a.turns() == 1);
        *asked_at.lock().unwrap() = Some(Instant::now());
        person.act(Some(Action::QuitNow), false);
        person.until("the quit", |_| quit_at.lock().unwrap().is_some());
    });
    let (asked, quit) = (
        asked_at.into_inner().unwrap(),
        quit_at.into_inner().unwrap(),
    );
    let waited = quit.unwrap() - asked.unwrap();
    assert!(waited >= Duration::from_millis(1900), "{waited:?}");
    assert!(waited < Duration::from_secs(3), "{waited:?}");
}

#[test]
fn quit_with_lanes_that_stop_ends_without_the_hard_exit() {
    let (to_a, a) = gated();
    let quit = || panic!("the lanes ended in time");
    at_desk_with(&[&a], Some("work"), &quit, |person| {
        person.until("A asks its model", |_| a.turns() == 1);
        person.act(Some(Action::QuitNow), false);
        to_a.send(call("read", json!({"path": "."}))).unwrap();
        person.until_over();
    });
}
