use contracts::history::Turn;

use super::*;

#[test]
fn a_session_round_trips_and_the_newest_comes_first() {
    let dir = std::env::temp_dir().join(format!("sessions-test-{}", now()));
    let sessions = Sessions { dir: dir.clone() };
    let mut old = sessions.start();
    old.id = "1-old".into();
    old.updated = 1;
    sessions.save(&old).unwrap();
    let mut new = sessions.start();
    new.record(
        Turn {
            goal: "find a quiz".into(),
            answer: "Done.".into(),
            status: "achieved".into(),
            at: now(),
        },
        Some("https://a.example/"),
    );
    sessions.save(&new).unwrap();
    let listed: Vec<String> = sessions.list().into_iter().map(|s| s.id).collect();
    assert_eq!(listed, [new.id.clone(), "1-old".into()]);
    let back = sessions.load(None).unwrap();
    assert_eq!(back.location, "https://a.example/");
    assert_eq!(back.turns[0].goal, "find a quiz");
    assert!(sessions.load(Some("missing")).is_err());
    let _ = std::fs::remove_dir_all(dir);
}
