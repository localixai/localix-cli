use std::time::Instant;

use contracts::tool::ToolOutput;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

use super::*;
use crate::tui::test_support::{Term, press, typed};

fn app() -> App {
    App::new(60, 20, vec![])
}

fn in_agent(app: &mut App, id: &str, event: Event, now: Instant) {
    app.event(Event::In(id.into(), Box::new(event)), now);
}

/// A main session that started `scout` (session `id`), which ran one call.
fn with_scout_as(id: &str, now: Instant) -> App {
    let mut app = app();
    app.event(Event::Note("main says hi".into()), now);
    app.event(
        Event::Started {
            id: id.into(),
            name: "scout".into(),
        },
        now,
    );
    in_agent(&mut app, id, Event::TaskStarted("find it".into()), now);
    in_agent(
        &mut app,
        id,
        Event::Result {
            name: "read".into(),
            args: json!({"path": "src/lib.rs"}),
            outcome: Ok(ToolOutput::default()),
        },
        now,
    );
    app
}

fn with_scout(now: Instant) -> App {
    with_scout_as("s1", now)
}

fn keys(app: &mut App, codes: &[KeyCode], now: Instant) {
    for &code in codes {
        press(app, code, now);
    }
}

/// A row of the tree chosen with `❯` (`❯ ○ scout …`): its session's name.
fn tree_row(line: &str) -> Option<&str> {
    let mut words = line.strip_prefix("❯ ")?.split_whitespace();
    let glyph = words.next()?;
    (glyph.chars().count() == 1).then_some(())?;
    words.next()
}

/// The session chosen in the tree, while the list has the keys.
fn chosen(screen: &str) -> Option<&str> {
    screen.lines().find_map(tree_row)
}

/// The composer's row: the last `❯` row that is not one of the tree's.
fn composer(screen: &str) -> &str {
    screen
        .lines()
        .rfind(|line| line.starts_with("❯ ") && tree_row(line).is_none())
        .unwrap_or("")
}

/// The tree's last row: how to use it now.
fn hint(screen: &str) -> &str {
    screen.lines().last().unwrap_or("").trim()
}

#[test]
fn an_agents_transcript_waits_in_its_view_until_it_is_shown() {
    let now = Instant::now();
    let mut app = with_scout(now);
    let mut term = Term::new(&app);
    let main = term.draw(&mut app, now);
    assert!(
        main.contains("main says hi") && !main.contains("src/lib.rs"),
        "{main}"
    );
    // ↓ from the empty prompt goes into the list, on main; ↓ again to scout, Enter shows it.
    press(&mut app, KeyCode::Down, now);
    let list = term.draw(&mut app, now);
    assert_eq!(chosen(&list), Some("main"), "{list}");
    keys(&mut app, &[KeyCode::Down, KeyCode::Enter], now);
    let scout = term.draw(&mut app, now);
    assert!(scout.starts_with("◆ agent scout  ·  session s1"), "{scout}");
    assert!(
        scout.contains("find it") && scout.contains("Read src/lib.rs"),
        "{scout}"
    );
    assert!(
        !scout.contains("main says hi"),
        "the screen was cleared:\n{scout}"
    );
    assert!(
        composer(&scout).starts_with("❯ Viewing agent scout"),
        "{scout}"
    );
    // While it is shown, what it does next is printed at once; the main session's is kept.
    in_agent(&mut app, "s1", Event::Answer("Found.".into()), now);
    app.event(Event::Note("main again".into()), now);
    let more = term.draw(&mut app, now);
    assert!(
        more.contains("Found.") && !more.contains("main again"),
        "{more}"
    );
    // ← goes back to main, and gives the keys back to the composer.
    press(&mut app, KeyCode::Left, now);
    let back = term.draw(&mut app, now);
    assert!(
        back.contains("main says hi") && back.contains("main again"),
        "{back}"
    );
    assert!(!back.contains("Found."), "{back}");
    assert_eq!(
        (chosen(&back), hint(&back)),
        (None, "↓ to manage · ← sessions"),
        "{back}"
    );
    assert!(composer(&back).starts_with("❯ Type a task"), "{back}");
}

#[test]
fn the_list_takes_the_keys_only_from_an_empty_prompt_and_gives_them_back() {
    let now = Instant::now();
    let mut app = with_scout(now);
    let mut term = Term::new(&app);
    keys(&mut app, &[KeyCode::Char('x'), KeyCode::Down], now);
    let screen = term.draw(&mut app, now);
    assert_eq!(
        chosen(&screen),
        None,
        "with text, ↓ is the composer's:\n{screen}"
    );
    assert_eq!(composer(&screen), "❯ x");
    press(&mut app, KeyCode::Backspace, now);
    keys(&mut app, &[KeyCode::Down; 3], now);
    let screen = term.draw(&mut app, now);
    assert_eq!(
        chosen(&screen),
        Some("scout"),
        "the last row stays chosen:\n{screen}"
    );
    assert_eq!(hint(&screen), "↑↓ choose · enter view · x stop · esc back");
    press(&mut app, KeyCode::Right, now);
    let screen = term.draw(&mut app, now);
    assert!(
        screen.starts_with("◆ agent scout"),
        "→ shows it too:\n{screen}"
    );
    keys(&mut app, &[KeyCode::Up, KeyCode::Up], now);
    let screen = term.draw(&mut app, now);
    assert_eq!(
        chosen(&screen),
        None,
        "↑ past the top gives the keys back:\n{screen}"
    );
    assert_eq!(hint(&screen), "↓ to manage · esc back to main");
    press(&mut app, KeyCode::Down, now);
    let screen = term.draw(&mut app, now);
    assert_eq!(
        chosen(&screen),
        Some("scout"),
        "back in the list on the session shown"
    );
    press(&mut app, KeyCode::Char('h'), now);
    let screen = term.draw(&mut app, now);
    assert_eq!(chosen(&screen), None, "{screen}");
    assert_eq!(composer(&screen), "❯ h", "typing goes to the composer");
}

#[test]
fn x_stops_the_agent_chosen() {
    let now = Instant::now();
    // The stops are known process-wide: an id no other test uses.
    let id = "tui-x-stops-the-agent-chosen";
    let mut app = with_scout_as(id, now);
    let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let _known = crate::interrupt::register_flag(id, flag.clone());
    keys(
        &mut app,
        &[KeyCode::Down, KeyCode::Down, KeyCode::Char('x')],
        now,
    );
    assert!(flag.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn esc_and_a_sent_entry_go_back_to_main() {
    let now = Instant::now();
    let mut app = with_scout(now);
    let mut term = Term::new(&app);
    keys(
        &mut app,
        &[KeyCode::Down, KeyCode::Down, KeyCode::Enter],
        now,
    );
    // The first Esc gives the list's keys back; the second goes back to main.
    press(&mut app, KeyCode::Esc, now);
    let screen = term.draw(&mut app, now);
    assert!(screen.starts_with("◆ agent scout"), "{screen}");
    press(&mut app, KeyCode::Esc, now);
    let screen = term.draw(&mut app, now);
    assert!(screen.contains("main says hi"), "{screen}");
    assert!(!screen.contains("◆ agent scout"), "{screen}");
    keys(
        &mut app,
        &[KeyCode::Down, KeyCode::Down, KeyCode::Enter],
        now,
    );
    typed(&mut app, "y", now);
    press(&mut app, KeyCode::Enter, now);
    let screen = term.draw(&mut app, now);
    assert!(!screen.contains("◆ agent scout"), "{screen}");
    assert!(screen.contains("main says hi"), "{screen}");
    assert!(
        screen.contains("  ⏎ y"),
        "the entry waits for the session:\n{screen}"
    );
}

#[test]
fn an_agent_ends_is_closed_and_is_cleared_with_its_conversation() {
    let now = Instant::now();
    let mut app = with_scout(now);
    let mut term = Term::new(&app);
    in_agent(
        &mut app,
        "s1",
        Event::TaskEnded {
            status: "achieved".into(),
            millis: 10,
            turns: 2,
            cost: 0.0,
        },
        now,
    );
    let screen = term.draw(&mut app, now);
    assert!(screen.contains("● Agent \"scout\" finished"), "{screen}");
    assert!(screen.contains("  ✓ scout  done"), "{screen}");
    assert!(!app.moving());
    app.event(Event::In("s1".into(), Box::new(Event::Closed)), now);
    let screen = term.draw(&mut app, now);
    assert!(screen.contains("  ○ scout  closed"), "{screen}");
    keys(
        &mut app,
        &[KeyCode::Down, KeyCode::Down, KeyCode::Enter],
        now,
    );
    assert!(term.draw(&mut app, now).starts_with("◆ agent scout"));
    app.event(Event::AgentsCleared, now);
    let screen = term.draw(&mut app, now);
    assert!(
        !screen.contains("○ scout") && !screen.contains("manage"),
        "{screen}"
    );
    assert!(screen.contains("main says hi"), "back on main:\n{screen}");
    assert!(composer(&screen).starts_with("❯ Type a task"), "{screen}");
}

#[test]
fn esc_in_an_agents_view_goes_back_before_it_stops_anything() {
    let now = Instant::now();
    let mut app = with_scout(now);
    app.event(Event::TaskStarted("main task".into()), now);
    app.show("s1");
    let esc = || Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(app.event(esc(), now), None);
    assert!(app.shown().parent.is_none());
    assert_eq!(
        app.event(esc(), now),
        Some(crate::tui::app::Action::Stop(String::new()))
    );
}

fn job(id: &str, name: &str) -> super::super::event::JobRow {
    super::super::event::JobRow {
        id: id.into(),
        name: name.into(),
        detail: format!("say {name}"),
        until: None,
        next: Some(0),
        events: 0,
        lane: String::new(),
    }
}

#[test]
fn the_list_chooses_only_job_rows_it_draws_and_keeps_a_row_chosen_when_rows_go() {
    let now = Instant::now();
    let mut app = app();
    let ids: Vec<String> = (1..=6)
        .map(|n| format!("tui-list-chooses-drawn-rows-{n}"))
        .collect();
    let rows = ids
        .iter()
        .enumerate()
        .map(|(n, id)| job(id, &format!("job{}", n + 1)))
        .collect();
    app.event(Event::Jobs(rows), now);
    let hidden = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let key = crate::interrupt::job_key("", &ids[5]);
    let _known = crate::interrupt::register_flag(&key, hidden.clone());
    // Into the list, then past its last drawn row.
    keys(&mut app, &[KeyCode::Down; 10], now);
    let screen = Term::new(&app).draw(&mut app, now);
    assert_eq!(chosen(&screen), Some("job5"), "{screen}");
    press(&mut app, KeyCode::Char('x'), now);
    assert!(!hidden.load(std::sync::atomic::Ordering::SeqCst));
    // The chosen job ends: the row chosen is one that is still there.
    app.event(Event::Jobs(vec![job(&ids[0], "job1")]), now);
    let screen = Term::new(&app).draw(&mut app, now);
    assert_eq!(chosen(&screen), Some("job1"), "{screen}");
}

#[test]
fn an_agent_started_in_a_lane_gets_its_events() {
    let now = Instant::now();
    let mut app = app();
    let lane = |event| Event::In("lane".into(), Box::new(event));
    app.event(
        Event::LaneStarted {
            id: "lane".into(),
            name: String::new(),
        },
        now,
    );
    let started = Event::Started {
        id: "s1".into(),
        name: "scout".into(),
    };
    app.event(lane(started), now);
    let found = Event::In("s1".into(), Box::new(Event::TaskStarted("look".into())));
    app.event(lane(found), now);
    let mut term = Term::new(&app);
    let screen = term.draw(&mut app, now);
    assert!(screen.contains("scout  thinking"), "{screen}");
    keys(
        &mut app,
        &[KeyCode::Down, KeyCode::Down, KeyCode::Enter],
        now,
    );
    let screen = term.draw(&mut app, now);
    assert!(screen.contains("look"), "{screen}");
}

#[test]
fn tagged_events_reach_their_own_view() {
    let now = Instant::now();
    let mut app = app();
    for id in ["A", "B"] {
        let name = format!("lane {id}");
        app.event(
            Event::LaneStarted {
                id: id.into(),
                name,
            },
            now,
        );
    }
    let lane = |id: &str, event| Event::In(id.into(), Box::new(event));
    let call = Event::Call {
        name: "read".into(),
        args: json!({"path": "a.rs"}),
    };
    app.event(lane("A", call), now);
    app.event(lane("B", Event::Answer("from b".into())), now);
    let (a, b) = (app.views.find("A").unwrap(), app.views.find("B").unwrap());
    let (a, b) = (&app.views.sessions[a], &app.views.sessions[b]);
    assert_eq!((a.running.len(), a.blocks.len()), (1, 0));
    assert_eq!((b.running.len(), b.blocks.len()), (0, 1));
    let text = |lines: Vec<Line>| lines.iter().map(ToString::to_string).collect::<String>();
    assert!(!text(app.take_history()).contains("from b"), "A is shown");
    app.show("B");
    assert!(text(app.take_history()).contains("from b"));
}

#[test]
fn clearing_a_hidden_lanes_agents_leaves_the_lane_shown() {
    let now = Instant::now();
    let mut app = app();
    for id in ["A", "B"] {
        let name = format!("lane {id}");
        app.event(
            Event::LaneStarted {
                id: id.into(),
                name,
            },
            now,
        );
    }
    let started = Event::Started {
        id: "scout".into(),
        name: "scout".into(),
    };
    app.event(Event::In("A".into(), Box::new(started)), now);
    app.show("B");
    app.take_replay();
    app.event(Event::In("A".into(), Box::new(Event::AgentsCleared)), now);
    assert_eq!(app.views.shown, "B", "the person stays where they are");
    assert!(!app.take_replay());
    assert!(app.views.find("scout").is_none(), "A's agent is gone");
}

#[test]
fn each_sessions_jobs_are_kept_apart_and_stopped_by_their_lane() {
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    for id in ["jobs-lane-a", "jobs-lane-b"] {
        let (id, name) = (id.to_string(), id.to_string());
        app.event(Event::LaneStarted { id, name }, now);
    }
    let from = |id: &str, rows| Event::In(id.into(), Box::new(Event::Jobs(rows)));
    app.event(from("jobs-lane-a", vec![job("job-1", "a-watch")]), now);
    app.event(from("jobs-lane-b", vec![job("job-1", "b-watch")]), now);
    let names =
        |app: &App| -> Vec<String> { app.views.jobs.iter().map(|j| j.name.clone()).collect() };
    assert_eq!(
        names(&app),
        ["a-watch", "b-watch"],
        "the last to report does not win"
    );
    let (a, b) = (
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    );
    let key = |lane| crate::interrupt::job_key(lane, "job-1");
    let _a = crate::interrupt::register_flag(&key("jobs-lane-a"), a.clone());
    let _b = crate::interrupt::register_flag(&key("jobs-lane-b"), b.clone());
    // Into the list: two lanes, then B's job.
    keys(
        &mut app,
        &[KeyCode::Down, KeyCode::Down, KeyCode::Down, KeyCode::Down],
        now,
    );
    press(&mut app, KeyCode::Char('x'), now);
    assert!(b.load(std::sync::atomic::Ordering::SeqCst), "B's job-1");
    assert!(
        !a.load(std::sync::atomic::Ordering::SeqCst),
        "not A's job-1"
    );
    app.event(from("jobs-lane-a", vec![]), now);
    assert_eq!(names(&app), ["b-watch"]);
}

#[test]
fn an_agents_view_names_the_session_that_started_it() {
    let now = Instant::now();
    let mut app = App::new(80, 20, vec![]);
    for (id, name) in [("lane-a", "first"), ("lane-b", "fix the flaky jobs test")] {
        let (id, name) = (id.to_string(), name.to_string());
        app.event(Event::LaneStarted { id, name }, now);
    }
    let started = Event::Started {
        id: "s9".into(),
        name: "scout".into(),
    };
    app.event(Event::In("lane-b".into(), Box::new(started)), now);
    app.show("s9");
    let shown = crate::tui::test_support::history(&mut app);
    assert!(
        shown.contains("started by fix the flaky jobs test"),
        "{shown}"
    );
}

#[test]
fn a_lane_is_named_by_its_first_task() {
    let now = Instant::now();
    let mut app = App::new(80, 20, vec![]);
    let (id, name) = ("lane-n".to_string(), String::new());
    app.event(Event::LaneStarted { id, name }, now);
    let task = |goal: &str| Event::In("lane-n".into(), Box::new(Event::TaskStarted(goal.into())));
    app.event(task("fix the flaky jobs test\nand more"), now);
    app.event(task("something else"), now);
    assert_eq!(app.shown().name, "fix the flaky jobs test");
}

#[test]
fn hidden_lane_done_sets_unseen_and_notes_it() {
    let now = Instant::now();
    let mut app = App::new(80, 20, vec![]);
    for (id, name) in [("A", "first"), ("B", "fix the flaky jobs test")] {
        let (id, name) = (id.to_string(), name.to_string());
        app.event(Event::LaneStarted { id, name }, now);
    }
    crate::tui::test_support::history(&mut app);
    let from = |event| Event::In("B".into(), Box::new(event));
    app.event(from(Event::TaskStarted("fix it".into())), now);
    let ended = Event::TaskEnded {
        status: "achieved".into(),
        millis: 188_000,
        turns: 3,
        cost: 0.0,
    };
    app.event(from(ended), now);
    let shown = crate::tui::test_support::history(&mut app);
    assert!(
        shown.contains("● Session \"fix the flaky jobs test\" finished · 3m 08s · ← to open"),
        "{shown}"
    );
    let b = app.views.find("B").unwrap();
    assert!(app.views.sessions[b].unseen);
    app.show("B");
    assert!(!app.views.sessions[b].unseen, "seen once shown");
}

#[test]
fn a_resumed_lane_is_named_by_the_first_line_of_its_first_task() {
    let now = Instant::now();
    let mut app = App::new(80, 20, vec![]);
    let (id, name) = ("first".to_string(), String::new());
    app.event(Event::LaneStarted { id, name }, now);
    let (id, name) = (
        "resumed".to_string(),
        "port the tests\nall of them".to_string(),
    );
    app.event(Event::LaneStarted { id, name }, now);
    let at = app.views.find("resumed").unwrap();
    assert_eq!(app.views.sessions[at].name, "port the tests");
}
