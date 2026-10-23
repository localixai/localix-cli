use std::time::Instant;

use agent::observer::Observer;
use crossterm::event::KeyCode;

use super::Watch;
use crate::output::Output;
use crate::testing::{Captured, fake_ui};
use crate::tui::app::App;
use crate::tui::test_support::{Term, press};
use protocol::Event;

#[test]
fn what_an_agent_says_beside_its_calls_shows_in_its_own_view() {
    let (ui, events) = fake_ui();
    let output = Output::with(
        false,
        false,
        Box::new(Captured::default()),
        Box::new(Captured::default()),
    )
    .in_ui(Some(ui));
    let watch = Watch {
        output: &output,
        id: "s1",
        name: "scout",
    };
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    let started = Event::Started {
        id: "s1".into(),
        name: "scout".into(),
    };
    app.event(started, now);
    watch.said("Checking the lockfile first.");
    for frame in events.try_iter() {
        app.frame(frame, now);
    }
    let mut term = Term::new(&app);
    let main = term.draw(&mut app, now);
    assert!(!main.contains("Checking the lockfile"), "{main}");
    for code in [KeyCode::Down, KeyCode::Down, KeyCode::Enter] {
        press(&mut app, code, now);
    }
    let scout = term.draw(&mut app, now);
    assert!(scout.contains("Checking the lockfile first."), "{scout}");
}
