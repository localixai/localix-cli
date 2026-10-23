use contracts::history::History;
use serde_json::json;

use super::test_support::{at_desk, call, ended, last_result, state};
use super::{Request, Target};
use crate::testing::{gated, says};
use crate::tui::views::AgentState;

#[test]
fn lane_a_keeps_running_while_lane_b_runs_a_task() {
    let ((to_a, a), (to_b, b)) = (gated(), gated());
    at_desk(&[&a, &b], Some("a's task"), |person| {
        person.until("A asks its model", |_| a.turns() == 1);
        let lane_a = person.first();
        let lane_b = person.new_lane();
        assert_ne!(lane_a, lane_b);
        person.enter("b's task");
        person.until("B asks its model", |_| b.turns() == 1);
        to_b.send(says("B is done.")).unwrap();
        person.until("B ends", |app| ended(app, &lane_b));
        assert_eq!(state(&person.app, &lane_a), AgentState::Working);
        to_a.send(says("A is done.")).unwrap();
        person.until("A ends", |app| ended(app, &lane_a));
    });
}

#[test]
fn each_lane_records_in_its_own_session() {
    let ((to_a, a), (to_b, b)) = (gated(), gated());
    let ids = std::sync::Mutex::new(vec![]);
    let scratch = at_desk(&[&a, &b], None, |person| {
        person.until("A waits", |app| app.lane().waiting.is_some());
        let lane_a = person.first();
        to_a.send(says("A is done.")).unwrap();
        person.enter("a's task");
        person.until("A ends", |app| ended(app, &lane_a));
        let lane_b = person.new_lane();
        to_b.send(says("B is done.")).unwrap();
        person.enter("b's task");
        person.until("B ends", |app| ended(app, &lane_b));
        *ids.lock().unwrap() = person.lanes();
    });
    let history = session::Sessions {
        dir: scratch.path().join("sessions"),
    };
    let goals = |id: &str| -> Vec<String> {
        let session = history.load(Some(id)).unwrap();
        session.turns.into_iter().map(|turn| turn.goal).collect()
    };
    let ids = ids.into_inner().unwrap();
    assert_eq!(goals(&ids[0]), ["a's task"]);
    assert_eq!(goals(&ids[1]), ["b's task"]);
}

#[test]
fn new_lane_leaves_the_old_lanes_agents_running() {
    let ((to_a, a), (_to_b, b)) = (gated(), gated());
    at_desk(&[&a, &b], Some("look around"), |person| {
        to_a.send(call("agents", json!({"task": "look", "name": "scout"})))
            .unwrap();
        // The agent and its parent each wait for a turn.
        person.until("the scout works", |app| {
            app.views.sessions.iter().any(|v| v.name == "scout") && a.turns() == 3
        });
        person.new_lane();
        let scout = person.app.views.sessions.iter().find(|v| v.name == "scout");
        assert_eq!(scout.unwrap().state, AgentState::Working);
        for _ in 0..4 {
            to_a.send(says("Done.")).unwrap();
        }
        let lane_a = person.first();
        person.until("A ends", |app| ended(app, &lane_a));
    });
}

#[cfg(unix)]
#[test]
fn lane_b_task_end_keeps_lane_a_shell() {
    let ((to_a, a), (to_b, b)) = (gated(), gated());
    at_desk(&[&a, &b], Some("start a shell"), |person| {
        let shell = json!({"action": "open", "kind": "terminal", "command": "sleep 30",
            "yield_ms": 100});
        to_a.send(call("window", shell)).unwrap();
        person.until("A's shell runs", |_| a.turns() == 2);
        let lane_b = person.new_lane();
        to_b.send(says("B is done.")).unwrap();
        person.enter("b's task");
        person.until("B ends", |app| ended(app, &lane_b));
        to_a.send(call(
            "window",
            json!({"action": "interact", "tab": "t1", "yield_ms": 100}),
        ))
        .unwrap();
        person.until("A reads its shell", |_| a.turns() == 3);
        let read = last_result(&a.asked.lock().unwrap()[2]);
        assert!(read.contains("session 1 running"), "{read}");
        to_a.send(says("A is done.")).unwrap();
        let lane_a = person.first();
        person.until("A ends", |app| ended(app, &lane_a));
    });
}

#[cfg(unix)]
#[test]
fn a_monitor_wakes_only_its_lane() {
    let ((to_a, a), (_to_b, b)) = (gated(), gated());
    at_desk(&[&a, &b], Some("watch"), |person| {
        let monitor = json!({"action": "monitor", "command": "sleep 1; echo up", "name": "m"});
        to_a.send(call("jobs", monitor)).unwrap();
        to_a.send(says("Watching.")).unwrap();
        let lane_a = person.first();
        person.until("A ends", |app| ended(app, &lane_a));
        person.new_lane();
        person.until("the monitor wakes A", |_| a.turns() == 3);
        let woken = a.asked.lock().unwrap()[2].clone();
        assert!(format!("{woken:?}").contains("up"), "{woken:?}");
        assert_eq!(b.turns(), 0, "B heard nothing");
        to_a.send(says("Seen.")).unwrap();
    });
}

#[test]
fn opening_a_live_session_shows_it() {
    let ((_to_a, a), (_to_b, b)) = (gated(), gated());
    at_desk(&[&a, &b], None, |person| {
        person.until("A waits", |app| app.lane().waiting.is_some());
        let lane_a = person.first();
        person.new_lane();
        let open = Request::Open(Target::Saved(lane_a.clone()));
        person.requests.send(open).unwrap();
        person.until("A shown", |app| app.views.shown == lane_a);
        person.wait(300);
        assert_eq!(person.lanes().len(), 2, "no second lane for A");
    });
}

#[test]
fn ninth_lane_closes_oldest_idle() {
    let chats: Vec<_> = (0..super::MAX_LANES + 1).map(|_| gated()).collect();
    let models: Vec<_> = chats.iter().map(|(_, chat)| chat).collect();
    at_desk(&models, None, |person| {
        person.until("A waits", |app| app.lane().waiting.is_some());
        let oldest = person.first();
        for _ in 1..super::MAX_LANES {
            person.new_lane();
        }
        assert_eq!(person.lanes().len(), super::MAX_LANES);
        let newest = person.new_lane();
        let lanes = person.lanes();
        assert_eq!(lanes.len(), super::MAX_LANES, "{lanes:?}");
        assert!(!lanes.contains(&oldest), "the oldest idle one closed");
        assert!(lanes.contains(&newest));
        let shown = crate::tui::test_support::history(&mut person.app);
        assert!(shown.contains("closed the oldest idle session"), "{shown}");
    });
}

#[test]
fn the_desk_never_opens_more_than_max_lanes() {
    let chats: Vec<_> = (0..super::MAX_LANES + 1).map(|_| gated()).collect();
    let models: Vec<_> = chats.iter().map(|(_, chat)| chat).collect();
    at_desk(&models, None, |person| {
        person.first();
        // Asked past the interface's own limit, as a request that raced it would be.
        for _ in 0..super::MAX_LANES {
            person.requests.send(Request::Open(Target::New)).unwrap();
        }
        person.until("the lanes open", |app| {
            let lanes = app
                .views
                .sessions
                .iter()
                .filter(|v| v.lane.is_some())
                .count();
            lanes == super::MAX_LANES
        });
        person.wait(1500);
        assert_eq!(person.lanes().len(), super::MAX_LANES);
    });
}
