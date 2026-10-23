use contracts::history::Turn;

use super::*;

/// A directory of its own for one test, by process and name, removed when the test ends
/// however it ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "llx-sessions-{}-{name}-{}",
            std::process::id(),
            now()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_session_round_trips_and_the_newest_comes_first() {
    let scratch = Scratch::new("round-trip");
    let sessions = Sessions {
        dir: scratch.0.clone(),
    };
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
            steps: vec![],
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
}

#[test]
fn the_newest_session_is_never_an_agent() {
    let scratch = Scratch::new("agent");
    let sessions = Sessions {
        dir: scratch.0.clone(),
    };
    let mut parent = sessions.start();
    parent.id = "1-parent".into();
    parent.updated = 1;
    sessions.save(&parent).unwrap();
    let mut agent = sessions.start();
    agent.id = "2-agent".into();
    agent.updated = 2;
    agent.parent = Some(parent.id.clone());
    agent.name = "scout".into();
    sessions.save(&agent).unwrap();
    assert_eq!(sessions.load(None).unwrap().id, "1-parent");
    let back = sessions.load(Some("2-agent")).unwrap();
    assert_eq!(
        (back.parent.as_deref(), back.name.as_str()),
        (Some("1-parent"), "scout")
    );
}
