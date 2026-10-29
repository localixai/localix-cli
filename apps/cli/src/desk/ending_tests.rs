use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::compose::{Models, Seat, Shared, lane_with};
use crate::desk::test_support::{Person, at_desk, at_desk_with, call, connected, ended, state};
use crate::desk::{Work, run};
use crate::server::serve::{Info, serve};
use crate::server::{Peer, Ui};
use crate::testing::{Scratch, gated, says, setup};
use crate::tui::app::{Action, App};
use crate::tui::views::AgentState;
use protocol::Command;

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
            .input(crate::tui::test_support::ctrl('c'), Instant::now());
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
        let (shared, seat) = setup(&scratch, crate::modes::built("bypass"), true).split();
        #[expect(
            clippy::disallowed_methods,
            reason = "the in-process transport and the desk's requests, as the binary makes them"
        )]
        let ((to_server, incoming), (out, frames), (to_desk, asked)) =
            (channel(), channel(), channel());
        let peer = Peer::new(out);
        let ui = Ui::new(Arc::clone(&peer));
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
                live: None,
            };
            lane_with(shared, seat, models, work)
        };
        std::thread::scope(|scope| {
            let desk = std::thread::Builder::new().name("desk".into());
            let desk = (desk.spawn_scoped(scope, || {
                run(&shared, &ui, asked, seat, None, &start, &|| {})
            }))
            .unwrap();
            let info = Info {
                max_lanes: crate::desk::MAX_LANES,
                commands: vec![],
                modes: vec![],
                host: None,
            };
            let serving = std::thread::Builder::new().name("serving".into());
            serving
                .spawn_scoped(scope, move || {
                    serve(None, &incoming, &peer, &to_desk, &info)
                })
                .unwrap();
            let mut person = Person {
                app: App::new(80, 24, vec![]),
                frames,
                to_server,
                over: false,
            };
            person.until("A waits", |app| app.lane().ready);
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
        // A call that outlasts the stop however late it lands (a quick one raced it to a turn).
        let long = call("bash", json!({"command": "sleep 30"}));
        to_a.send(long).unwrap();
        person.until_over();
    });
}

/// The command outlasts the stop, however late it lands: it is killed, and no turn comes after.
#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "nothing tells a lane that it was not asked: time passing does, bounded"
)]
fn a_connection_that_ends_without_background_stops_the_work() {
    let (to_a, a) = gated();
    connected(
        &[&a],
        Some("work"),
        &|| panic!("no hard exit"),
        |mut person, _| {
            person.until("A asks its model", |_| a.turns() == 1);
            // The terminal closed, nothing chosen: as "Exit and stop tasks".
            drop(person);
            std::thread::sleep(Duration::from_millis(300));
            to_a.send(call("read", json!({"path": "."}))).unwrap();
            std::thread::sleep(Duration::from_millis(500));
            let turns = a.turns();
            if turns > 1 {
                // Let the lane end, so the failure is told rather than waited on.
                to_a.send(says("Done.")).unwrap();
            }
            assert_eq!(turns, 1, "the task was stopped, not asked to go on");
        },
    );
}

#[test]
#[expect(
    clippy::disallowed_methods,
    reason = "nothing tells a lane that it was not asked: time passing does, bounded"
)]
fn moved_to_the_background_a_lane_waits_for_the_next_interface_and_keeps_its_jobs() {
    let (to_a, a) = gated();
    connected(
        &[&a],
        Some("remind me"),
        &|| panic!("no hard exit"),
        |mut person, peer| {
            let later =
                json!({"action": "schedule", "prompt": "check", "in": "2h", "name": "later"});
            to_a.send(call("jobs", later)).unwrap();
            to_a.send(says("Scheduled.")).unwrap();
            let lane = person.first();
            person.until("A waits", |app| ended(app, &lane) && app.lane().ready);
            person.command(Command::Background);
            drop(person);
            std::thread::sleep(Duration::from_millis(500));
            let kept = peer.work.running();
            let schedule = protocol::Running {
                kind: "schedule".into(),
                name: "later".into(),
            };
            // An entry from the next interface reaches the lane: its prompt still waits.
            to_a.send(says("Again.")).unwrap();
            let again = protocol::Input {
                session: lane,
                text: "again".into(),
                ..protocol::Input::default()
            };
            peer.inputs.input(again, false);
            let deadline = Instant::now() + Duration::from_secs(5);
            while a.turns() < 3 && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            let turns = a.turns();
            peer.inputs.leave();
            assert_eq!(kept, [schedule], "its schedule runs on");
            assert_eq!(turns, 3, "the lane took the next interface's entry");
        },
    );
}
