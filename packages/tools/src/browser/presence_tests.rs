use std::time::Duration;

use super::Effect::{Blink, Busy, Capture, Enter};
use super::Event::{
    Acting, Appeared, Changed, Collapse, Expand, Pictured, Settled, Show, Vanished,
};
use super::{Presence, State, Support, Timing};

fn mini() -> Presence {
    Presence::new(Support::Mini, Timing::default())
}

/// A presence that has seen its tabs appear and its first picture taken.
fn showing() -> Presence {
    let mut presence = mini();
    presence.event(Appeared);
    presence.event(Pictured);
    presence
}

#[test]
fn tabs_open_as_the_mini_and_ask_for_its_picture() {
    let mut presence = mini();
    assert_eq!(presence.state(), State::Hidden);
    assert_eq!(presence.event(Appeared), vec![Enter(State::Mini), Capture]);
    assert_eq!(presence.state(), State::Mini);
}

#[test]
fn without_a_mini_the_window_opens_ordinary() {
    let mut presence = Presence::new(Support::WindowOnly, Timing::default());
    assert_eq!(presence.event(Appeared), vec![Enter(State::Expanded)]);
    // Nothing collapses it, and no picture is kept.
    assert_eq!(presence.event(Collapse), vec![]);
    assert_eq!(presence.event(Settled), vec![]);
    assert_eq!(presence.state(), State::Expanded);
}

#[test]
fn only_the_person_expands_and_collapses() {
    let mut presence = showing();
    assert_eq!(presence.event(Expand), vec![Enter(State::Expanded)]);
    assert_eq!(presence.event(Expand), vec![]);
    assert_eq!(presence.event(Collapse), vec![Enter(State::Mini), Capture]);
    assert_eq!(presence.event(Collapse), vec![]);
}

#[test]
fn the_agent_never_expands_the_window() {
    let mut presence = showing();
    for event in [Acting, Show, Changed, Settled, Pictured] {
        let out = presence.event(event);
        assert!(!out.contains(&Enter(State::Expanded)), "{event:?}: {out:?}");
        assert_eq!(presence.state(), State::Mini, "{event:?}");
    }
}

#[test]
fn show_blinks_the_mini_and_takes_a_new_picture() {
    let mut presence = showing();
    let blink = Timing::default().blink;
    assert_eq!(presence.event(Show), vec![Blink(blink), Capture]);
    // Expanded, the person sees the tab itself: nothing to blink.
    presence.event(Expand);
    assert_eq!(presence.event(Show), vec![]);
}

#[test]
fn a_spinner_while_the_agent_acts_and_a_picture_when_it_settles() {
    let mut presence = showing();
    assert_eq!(presence.event(Acting), vec![Busy(true)]);
    assert_eq!(presence.event(Acting), vec![]);
    assert_eq!(presence.event(Settled), vec![Busy(false), Capture]);
    // The picture asked for is still being taken: the next one waits for it.
    assert_eq!(presence.event(Settled), vec![]);
}

#[test]
fn pictures_are_taken_one_at_a_time_and_the_newest_follows() {
    let mut presence = mini();
    assert_eq!(presence.event(Appeared), vec![Enter(State::Mini), Capture]);
    // Two changes while a picture is in flight: one more is taken, not two.
    assert_eq!(presence.event(Changed), vec![]);
    assert_eq!(presence.event(Settled), vec![]);
    assert_eq!(presence.event(Pictured), vec![Capture]);
    assert_eq!(presence.event(Pictured), vec![]);
}

#[test]
fn expanded_it_takes_no_pictures_and_collapsing_takes_one() {
    let mut presence = showing();
    presence.event(Expand);
    assert_eq!(presence.event(Settled), vec![]);
    assert_eq!(presence.event(Changed), vec![]);
    assert_eq!(presence.event(Collapse), vec![Enter(State::Mini), Capture]);
}

#[test]
fn no_tabs_hides_everything_and_starts_over() {
    let mut presence = showing();
    presence.event(Acting);
    assert_eq!(presence.event(Vanished), vec![Enter(State::Hidden)]);
    assert_eq!(presence.event(Vanished), vec![]);
    assert_eq!(presence.event(Settled), vec![]);
    // A later window opens as a mini again, with no spinner left over.
    assert_eq!(presence.event(Appeared), vec![Enter(State::Mini), Capture]);
    assert_eq!(presence.event(Settled), vec![]);
    assert_eq!(presence.event(Acting), vec![Busy(true)]);
}

#[test]
fn a_blink_is_as_long_as_it_is_told() {
    let timing = Timing {
        blink: Duration::from_millis(5),
    };
    let mut presence = Presence::new(Support::Mini, timing);
    presence.event(Appeared);
    presence.event(Pictured);
    assert_eq!(
        presence.event(Show),
        vec![Blink(Duration::from_millis(5)), Capture]
    );
}
