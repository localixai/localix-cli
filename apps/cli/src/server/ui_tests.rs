use super::*;
use crate::tui::test_support::fake_ui;

/// The session `event` came from, and what it was.
fn tagged(event: Event) -> (String, Event) {
    match event {
        Event::In(id, inner) => (id, *inner),
        _ => panic!("not tagged"),
    }
}

#[test]
fn a_lanes_ui_tags_what_it_sends() {
    let (ui, events) = fake_ui();
    ui.for_lane("a").send(Event::Note("hi".into()));
    let (id, inner) = tagged(events.try_recv().unwrap());
    assert_eq!(id, "a");
    assert!(matches!(inner, Event::Note(text) if text == "hi"));
}

#[test]
fn an_agents_event_keeps_its_own_tag() {
    let (ui, events) = fake_ui();
    let inner = Box::new(Event::Answer("found".into()));
    ui.for_lane("a").send(Event::In("agent".into(), inner));
    let (id, inner) = tagged(events.try_recv().unwrap());
    assert_eq!(id, "agent");
    assert!(matches!(inner, Event::Answer(_)));
}

#[test]
fn the_interfaces_own_ui_tags_nothing() {
    // What the interface itself sends (the keys' thread has its own sender, the end of the
    // session goes through this one) belongs to no session.
    let (ui, events) = fake_ui();
    ui.send(Event::Quit);
    assert!(matches!(events.try_recv().unwrap(), Event::Quit));
}

#[test]
fn a_question_closed_by_a_stop_is_dismissed_in_its_own_lane() {
    let (ui, events) = fake_ui();
    let answer = ui.for_lane("a").approve("bash", "ls", &|| true);
    assert_eq!(answer, Approval::No(None));
    let (id, asked) = tagged(events.try_recv().unwrap());
    assert!(id == "a" && matches!(asked, Event::Approve { .. }));
    let (id, closed) = tagged(events.try_recv().unwrap());
    assert!(id == "a" && matches!(closed, Event::Dismiss));
}
