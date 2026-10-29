use std::sync::mpsc::{Receiver, channel};

use protocol::{Asked, Event};

use super::*;

#[expect(
    clippy::disallowed_methods,
    reason = "a client's channel, as the in-process transport makes it"
)]
fn client() -> (Sender<ToClient>, Receiver<ToClient>) {
    channel()
}

fn note(text: &str) -> ToClient {
    ToClient::Event {
        session: Some("s1".into()),
        event: Event::Note(text.into()),
        seq: None,
    }
}

fn ask(id: u64) -> ToClient {
    ToClient::Ask {
        id,
        session: "s1".into(),
        asked: Asked::Approve {
            tool: "bash".into(),
            summary: "ls".into(),
        },
    }
}

/// The notes a client was sent, with their numbers; a gap as `"gap"`.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn notes(frames: &Receiver<ToClient>) -> Vec<(String, Option<u64>)> {
    (frames.try_iter())
        .filter_map(|frame| match frame {
            ToClient::Event {
                event: Event::Note(text),
                seq,
                ..
            } => Some((text, seq)),
            ToClient::Gap => Some(("gap".into(), None)),
            _ => None,
        })
        .collect()
}

#[test]
fn every_client_attached_gets_every_event_numbered() {
    let hub = Hub::new(10);
    let ((a, seen_a), (b, seen_b)) = (client(), client());
    hub.attach(a, Some(0));
    hub.attach(b, Some(0));
    hub.send(note("one"));
    hub.send(note("two"));
    let both = vec![("one".into(), Some(1)), ("two".into(), Some(2))];
    assert_eq!(notes(&seen_a), both);
    assert_eq!(notes(&seen_b), both);
    assert_eq!(hub.attached(), 2);
}

#[test]
fn a_client_that_attaches_later_gets_what_came_after_its_cursor_then_what_waits() {
    let hub = Hub::new(10);
    hub.send(note("one"));
    hub.send(note("two"));
    hub.send(ask(7));
    hub.send(note("three"));
    let (out, seen) = client();
    let (_, seq) = hub.attach(out, Some(1));
    assert_eq!(seq, 3);
    let frames: Vec<_> = seen.try_iter().collect();
    assert_eq!(frames.len(), 3, "{frames:?}");
    assert!(matches!(&frames[0], ToClient::Event { seq: Some(2), .. }));
    assert!(matches!(&frames[1], ToClient::Event { seq: Some(3), .. }));
    assert_eq!(frames[2], ask(7));
}

#[test]
fn a_question_answered_is_not_given_to_a_client_that_attaches_after() {
    let hub = Hub::new(10);
    hub.send(ask(7));
    hub.send(ToClient::Resolved { id: 7 });
    let (out, seen) = client();
    hub.attach(out, Some(0));
    assert_eq!(seen.try_iter().count(), 0);
}

#[test]
fn a_cursor_older_than_the_ring_gets_a_gap_first() {
    let hub = Hub::new(2);
    for text in ["one", "two", "three"] {
        hub.send(note(text));
    }
    let (out, seen) = client();
    hub.attach(out, Some(0));
    let after = vec![
        ("gap".into(), None),
        ("two".into(), Some(2)),
        ("three".into(), Some(3)),
    ];
    assert_eq!(notes(&seen), after);
}

#[test]
fn no_cursor_on_a_ring_that_wrapped_is_a_gap_too_and_one_that_did_not_is_not() {
    let hub = Hub::new(2);
    hub.send(note("one"));
    let (out, seen) = client();
    hub.attach(out, None);
    assert_eq!(notes(&seen), [("one".into(), Some(1))]);
    hub.send(note("two"));
    hub.send(note("three"));
    let (out, seen) = client();
    hub.attach(out, None);
    assert_eq!(notes(&seen)[0], ("gap".into(), None));
}

#[test]
fn a_cursor_from_another_server_gets_a_gap_and_everything_kept() {
    let hub = Hub::new(10);
    hub.send(note("one"));
    let (out, seen) = client();
    hub.attach(out, Some(99));
    let after = vec![("gap".into(), None), ("one".into(), Some(1))];
    assert_eq!(notes(&seen), after);
}

#[test]
fn live_only_events_reach_who_is_attached_and_are_never_replayed() {
    let hub = Hub::new(10);
    let (out, live) = client();
    hub.attach(out, None);
    for event in [Event::Step("click".into()), Event::Copy("x".into())] {
        hub.send(ToClient::Event {
            session: None,
            event,
            seq: None,
        });
    }
    let unnumbered = live
        .try_iter()
        .filter(|frame| matches!(frame, ToClient::Event { seq: None, .. }));
    assert_eq!(unnumbered.count(), 2);
    let (out, later) = client();
    let (_, seq) = hub.attach(out, None);
    assert_eq!((seq, later.try_iter().count()), (0, 0));
}

#[test]
fn a_client_gone_is_dropped_and_the_others_still_served() {
    let hub = Hub::new(10);
    let ((a, seen_a), (b, seen_b)) = (client(), client());
    hub.attach(a, None);
    hub.attach(b, None);
    drop(seen_a);
    hub.send(note("one"));
    hub.send(note("two"));
    assert_eq!(notes(&seen_b).len(), 2);
    assert_eq!(hub.attached(), 1);
}

#[test]
fn a_client_detached_hears_nothing_more() {
    let hub = Hub::new(10);
    let ((a, seen_a), (b, _seen_b)) = (client(), client());
    let (id, _) = hub.attach(a, None);
    hub.attach(b, None);
    assert_eq!(hub.detach(id), 1);
    hub.send(note("one"));
    assert!(notes(&seen_a).is_empty());
}

#[test]
fn a_client_attached_again_replays_after_its_new_cursor() {
    let hub = Hub::new(10);
    let (out, seen) = client();
    let (id, _) = hub.attach(out, None);
    hub.send(note("one"));
    hub.send(note("two"));
    assert_eq!(notes(&seen).len(), 2);
    assert_eq!(hub.rewind(id, Some(1)), Some(2));
    assert_eq!(notes(&seen), [("two".into(), Some(2))]);
    hub.send(note("three"));
    assert_eq!(notes(&seen), [("three".into(), Some(3))]);
}

#[test]
fn a_frame_for_one_client_reaches_it_alone() {
    let hub = Hub::new(10);
    let ((a, seen_a), (b, seen_b)) = (client(), client());
    let (id, _) = hub.attach(a, None);
    hub.attach(b, None);
    let done = ToClient::Done {
        id: 1,
        done: protocol::Done::Ok,
    };
    hub.to(id, done.clone());
    assert_eq!(seen_a.try_iter().collect::<Vec<_>>(), [done]);
    assert_eq!(seen_b.try_iter().count(), 0);
}

#[test]
fn an_event_still_on_its_way_reaches_a_client_attaching_meanwhile_once() {
    let hub = Hub::new(10);
    // Another thread is sending: what is posted waits for it.
    hub.sending.store(true, Ordering::Release);
    hub.send(note("one"));
    let (out, seen) = client();
    hub.attach(out, Some(0));
    hub.sending.store(false, Ordering::Release);
    hub.flush();
    assert_eq!(notes(&seen), [("one".into(), Some(1))]);
}

fn said(session: Option<&str>, event: Event) -> ToClient {
    ToClient::Event {
        session: session.map(str::to_string),
        event,
        seq: None,
    }
}

fn queue(queued: &[&str]) -> Event {
    Event::Queue {
        ready: true,
        steered: vec![],
        queued: queued.iter().map(|text| text.to_string()).collect(),
        background: vec![],
    }
}

/// The events a client was sent, without their numbers; a gap as `None`.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn events(frames: &Receiver<ToClient>) -> Vec<Option<(Option<String>, Event)>> {
    (frames.try_iter())
        .filter_map(|frame| match frame {
            ToClient::Event { session, event, .. } => Some(Some((session, event))),
            ToClient::Gap => Some(None),
            _ => None,
        })
        .collect()
}

/// A lane `s1` with a mode, a queue, a total and an agent `a1`, then `notes` notes.
fn a_lane_then(hub: &Hub, notes: usize) {
    let (id, name) = ("s1".to_string(), "fix it".to_string());
    hub.send(said(None, Event::LaneStarted { id, name }));
    hub.send(said(Some("s1"), Event::Header(vec!["llx".into()])));
    hub.send(said(Some("s1"), Event::Mode("plan".into())));
    hub.send(said(Some("s1"), queue(&["later"])));
    hub.send(said(Some("s1"), Event::Total(protocol::Total::default())));
    let (id, name) = ("a1".to_string(), "scout".to_string());
    hub.send(said(Some("s1"), Event::Started { id, name }));
    hub.send(said(Some("a1"), Event::Closed));
    for at in 0..notes {
        hub.send(note(&format!("n{at}")));
    }
}

#[test]
fn a_ring_that_wrapped_still_gives_each_lane_whole_then_what_it_kept() {
    let hub = Hub::new(3);
    a_lane_then(&hub, 10);
    let (out, seen) = client();
    hub.attach(out, None);
    let seen = events(&seen);
    let (id, name) = ("s1".to_string(), "fix it".to_string());
    let s1 = |event| Some((Some("s1".to_string()), event));
    let mut wanted = vec![
        None,
        Some((None, Event::LaneStarted { id, name })),
        s1(Event::Header(vec!["llx".into()])),
        s1(Event::Mode("plan".into())),
        s1(queue(&["later"])),
        s1(Event::Total(protocol::Total::default())),
    ];
    let (id, name) = ("a1".to_string(), "scout".to_string());
    wanted.push(s1(Event::Started { id, name }));
    wanted.push(Some((Some("a1".to_string()), Event::Closed)));
    for at in 7..10 {
        wanted.push(s1(Event::Note(format!("n{at}"))));
    }
    assert_eq!(seen, wanted);
}

#[test]
fn of_a_whole_state_only_the_latest_is_kept() {
    let hub = Hub::new(100);
    for queued in [&["a"][..], &["a", "b"], &[]] {
        hub.send(said(Some("s1"), queue(queued)));
    }
    let (out, seen) = client();
    hub.attach(out, None);
    assert_eq!(events(&seen), [Some((Some("s1".into()), queue(&[])))]);
}

#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn a_lane_that_ended_is_not_given_again_nor_its_agents() {
    let hub = Hub::new(100);
    a_lane_then(&hub, 2);
    let (id, name) = ("s2".to_string(), "other".to_string());
    hub.send(said(None, Event::LaneStarted { id, name }));
    hub.send(said(Some("s1"), Event::LaneEnded));
    let (out, seen) = client();
    hub.attach(out, None);
    let sessions: Vec<_> = (events(&seen).into_iter().flatten())
        .filter(|(_, event)| *event != Event::LaneEnded)
        .map(|(session, event)| match event {
            Event::LaneStarted { id, .. } => id,
            _ => session.unwrap_or_default(),
        })
        .collect();
    assert_eq!(sessions, ["s2"]);
}

#[test]
fn the_ring_keeps_no_more_than_its_bytes_and_says_the_gap() {
    let hub = Hub::with_bytes(100, 10_000);
    let report = "x".repeat(3_000);
    for call in 0..10 {
        hub.send(said(
            Some("s1"),
            Event::Result {
                call: call.to_string(),
                name: "bash".into(),
                label: protocol::Label::default(),
                outcome: protocol::Outcome::Done {
                    report: report.clone(),
                    shown: String::new(),
                },
            },
        ));
    }
    let (out, seen) = client();
    hub.attach(out, None);
    let seen = events(&seen);
    assert_eq!(seen[0], None, "a gap first");
    let kept = seen.len() - 1;
    assert!(
        (1..=3).contains(&kept),
        "{kept} reports of 3,000 bytes kept in 10,000"
    );
}

#[test]
fn a_call_s_label_counts_toward_the_ring_s_bytes_at_its_start_and_at_its_result() {
    let hub = Hub::with_bytes(100, 10_000);
    // A bash call's label is its whole command, said at its start and again with its result.
    let label = protocol::Label {
        doing: "Running".into(),
        done: "Ran".into(),
        subject: "x".repeat(3_000),
    };
    for call in 0..5 {
        let (call, name) = (call.to_string(), "bash".to_string());
        let started = Event::Call {
            call: call.clone(),
            name: name.clone(),
            label: label.clone(),
        };
        hub.send(said(Some("s1"), started));
        let ended = Event::Result {
            call,
            name,
            label: label.clone(),
            outcome: protocol::Outcome::Done {
                report: "ok".into(),
                shown: String::new(),
            },
        };
        hub.send(said(Some("s1"), ended));
    }
    let (out, seen) = client();
    hub.attach(out, None);
    let seen = events(&seen);
    assert!(
        seen[0].is_none(),
        "a gap first: all {} events kept",
        seen.len()
    );
    let kept = seen.len() - 1;
    assert!(
        (1..=3).contains(&kept),
        "{kept} events of 3,000 bytes kept in 10,000"
    );
}
