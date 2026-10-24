use super::*;
use crate::testing::fake_ui;

#[test]
fn a_lanes_ui_tags_what_it_sends() {
    let (ui, frames) = fake_ui();
    ui.for_lane("a").send(Event::Note("hi".into()));
    let frame = frames.try_recv().unwrap();
    let note = ToClient::Event {
        session: Some("a".into()),
        event: Event::Note("hi".into()),
        seq: Some(1),
    };
    assert_eq!(frame, note);
}

#[test]
fn an_agents_events_are_in_its_own_name() {
    let (ui, frames) = fake_ui();
    ui.for_lane("a")
        .for_lane("agent")
        .send(Event::Answer("found".into()));
    let frame = frames.try_recv().unwrap();
    let answer = ToClient::Event {
        session: Some("agent".into()),
        event: Event::Answer("found".into()),
        seq: Some(1),
    };
    assert_eq!(frame, answer);
}

#[test]
fn the_servers_own_ui_tags_nothing() {
    // What the server says of itself (the end of the work) belongs to no session.
    let (ui, frames) = fake_ui();
    ui.send(Event::Quit);
    let quit = ToClient::Event {
        session: None,
        event: Event::Quit,
        seq: None,
    };
    assert_eq!(frames.try_recv().unwrap(), quit);
}
