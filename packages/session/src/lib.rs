//! A history store: each session one JSON file in a folder the app chooses, written whole to a
//! temporary file and renamed, so a crash never leaves half a session.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use agent::history::{History, Session};

/// Unix seconds now.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `3 min ago`, `2 h ago`, `5 d ago`.
pub fn ago(at: u64) -> String {
    let seconds = now().saturating_sub(at);
    match seconds {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", seconds / 60),
        3600..86400 => format!("{} h ago", seconds / 3600),
        _ => format!("{} d ago", seconds / 86400),
    }
}

/// Sessions as files in `dir`.
pub struct Sessions {
    pub dir: PathBuf,
}

impl History for Sessions {
    fn start(&self) -> Session {
        let mut random = [0u8; 2];
        let _ = getrandom::fill(&mut random);
        let created = now();
        Session {
            id: format!("{created}-{:02x}{:02x}", random[0], random[1]),
            created,
            updated: created,
            ..Session::default()
        }
    }

    fn save(&self, session: &Session) -> Result<(), String> {
        let write = || -> std::io::Result<()> {
            std::fs::create_dir_all(&self.dir)?;
            let path = self.dir.join(format!("{}.json", session.id));
            let partial = self.dir.join(format!(".{}.json.part", session.id));
            std::fs::write(&partial, serde_json::to_vec(session)?)?;
            std::fs::rename(&partial, &path)
        };
        write().map_err(|error| error.to_string())
    }

    fn load(&self, id: Option<&str>) -> Result<Session, String> {
        let id = match id {
            Some(id) => id.to_string(),
            None => self
                .list()
                .first()
                .map(|session| session.id.clone())
                .ok_or("no saved sessions")?,
        };
        let text = std::fs::read_to_string(self.dir.join(format!("{id}.json")))
            .map_err(|_| format!("no session {id}"))?;
        serde_json::from_str(&text).map_err(|error| format!("session {id} is unreadable: {error}"))
    }

    fn list(&self) -> Vec<Session> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return vec![];
        };
        let mut sessions: Vec<Session> = entries
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|e| e == "json"))
            .filter_map(|entry| std::fs::read_to_string(entry.path()).ok())
            .filter_map(|text| serde_json::from_str::<Session>(&text).ok())
            .collect();
        sessions.sort_by_key(|session| std::cmp::Reverse(session.updated));
        sessions
    }
}

#[cfg(test)]
mod tests {
    use agent::history::Turn;

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
                tool: Some("browser".into()),
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
}
