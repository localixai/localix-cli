use contracts::chat::Completion;
use serde_json::json;

use super::*;
use crate::compose::{Models, assemble};
use crate::interrupt::Interrupt;
use crate::testing::{Scratch, ScriptedChat, quiet, says};

/// A session opened from a client elsewhere: its commands run there, with what the client has
/// and this process lacks.
#[test]
fn a_session_works_in_the_clients_directory_with_the_clients_environment() {
    let (scratch, there) = (Scratch::new(), Scratch::new());
    there.file("marker.txt", "");
    let place = Place {
        cwd: there.path().display().to_string(),
        env: vec![
            ("PATH".into(), std::env::var("PATH").unwrap_or_default()),
            ("LLX_ONLY_THE_CLIENT_HAS".into(), "the client's".into()),
        ],
        ..Place::default()
    };
    assert!(std::env::var_os("LLX_ONLY_THE_CLIENT_HAS").is_none());
    let mut setup = crate::testing::setup(&scratch, crate::modes::built("bypass"), false);
    setup.workspace = workspace(&place).unwrap();
    let command = r#"ls; echo "[$LLX_ONLY_THE_CLIENT_HAS]""#;
    let chat = ScriptedChat::new(vec![
        Completion {
            calls: vec![crate::testing::scripted(
                "b",
                "bash",
                json!({ "command": command }),
            )],
            ..Completion::default()
        },
        says("done"),
    ]);
    let models = Models {
        chat: &chat,
        live: None,
    };
    let (shared, seat) = setup.split();
    let root = std::fs::canonicalize(there.path()).unwrap();
    assemble(&shared, seat, models, &quiet(), &Interrupt::new(), |rig| {
        assert_eq!(rig.workspace.root(), root);
        rig.task("what is there?");
        let shown = crate::testing::results(rig);
        let report = &shown[0].1;
        assert!(report.contains("marker.txt\n[the client's]"), "{report}");
        0
    });
}

#[test]
fn a_place_that_is_not_a_directory_is_refused() {
    let scratch = Scratch::new();
    scratch.file("a.txt", "");
    let cwd = scratch.path().join("a.txt").display().to_string();
    let place = Place {
        cwd,
        ..Place::default()
    };
    let why = workspace(&place).unwrap_err();
    assert!(why.ends_with("a.txt: not a directory"), "{why}");
}

#[test]
fn the_model_the_clients_environment_names_is_the_sessions_unless_its_command_line_says_another() {
    let place = |args: &[&str], env: &[(&str, &str)]| Place {
        args: args.iter().map(|arg| arg.to_string()).collect(),
        env: (env.iter())
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        ..Place::default()
    };
    let env = [
        ("LLX_PROVIDER", "openrouter"),
        ("LLX_MODEL", "anthropic/claude-haiku-5.5"),
        ("LLX_REASONING", "low"),
        ("HOME", "/home/a"),
    ];
    let said = |chosen: Vec<(&str, String)>| -> Vec<(String, String)> {
        chosen
            .into_iter()
            .map(|(var, value)| (var.to_string(), value))
            .collect()
    };
    assert_eq!(
        said(chosen(&place(&["resume", "1-ab"], &env))),
        [
            ("LLX_PROVIDER".to_string(), "openrouter".to_string()),
            ("LLX_MODEL".into(), "anthropic/claude-haiku-5.5".into()),
            ("LLX_REASONING".into(), "low".into()),
        ]
    );
    // What the command line says wins, in any of its spellings.
    let given = place(
        &["-mx/y", "--reasoning=high", "--provider", "deepseek"],
        &env,
    );
    assert!(chosen(&given).is_empty());
    // An empty variable names nothing.
    assert!(chosen(&place(&[], &[("LLX_MODEL", "")])).is_empty());
}
