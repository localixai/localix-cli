//! The binary end to end, with no network: every case stops before a model is called (a task
//! with a scripted model runs in `commands/run_tests.rs`). Each test gets its own home, config
//! and sessions directories, so nothing of the person's is read.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fresh, empty home for one test, removed when it ends.
struct Home(PathBuf);

impl Home {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "llx-e2e-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// `llx args…` with this home, no key in the environment and stdin closed.
    fn llx(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_llx"));
        command
            .args(args)
            .env("HOME", &self.0)
            .env("USERPROFILE", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .env("APPDATA", self.0.join("config"))
            .env("LLX_SESSIONS", self.0.join("sessions"))
            .env_remove("OPENROUTER_API_KEY")
            .env_remove("LLX_MODEL")
            .env_remove("LLX_PROVIDER")
            .env_remove("LLX_REASONING")
            .env_remove("LLX_DECISION_MODEL")
            .stdin(Stdio::null());
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.llx(args).output().unwrap()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

const FAKE_KEY: &str = "sk-or-v1-0000000000000000000000000000wxyz";

#[test]
fn help_and_version_exit_zero() {
    let home = Home::new();
    let help = home.run(&["--help"]);
    assert_eq!(help.status.code(), Some(0));
    let shown = text(&help.stdout);
    assert!(shown.contains("Usage: llx [OPTIONS] [PROMPT]"), "{shown}");
    assert!(shown.contains("Examples:"));
    let version = home.run(&["--version"]);
    assert_eq!(version.status.code(), Some(0));
    assert!(text(&version.stdout).starts_with("llx "));
}

#[test]
fn an_unknown_flag_is_a_usage_error() {
    let home = Home::new();
    let out = home.run(&["--no-such-flag", "x"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(text(&out.stderr).contains("--no-such-flag"));
    assert_eq!(home.run(&["resume"]).status.code(), Some(3));
    // -y is --mode bypass, so it cannot be given with another mode.
    let both = home.run(&["exec", "-y", "--mode", "plan", "x"]);
    assert_eq!(both.status.code(), Some(3));
    assert!(text(&both.stderr).contains("--mode"));
}

#[test]
fn signing_in_is_no_command_of_its_own() {
    let home = Home::new();
    // `llx login` is a session whose first message is "login": it needs a terminal.
    let out = home.run(&["login", "status"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(
        text(&out.stderr).contains("needs a terminal"),
        "{}",
        text(&out.stderr)
    );
    let help = text(&home.run(&["--help"]).stdout);
    assert!(
        help.contains("/login") && !help.contains("llx login"),
        "{help}"
    );
}

#[test]
fn logout_forgets_every_saved_sign_in_without_showing_it() {
    let home = Home::new();
    let config = home.path().join("config/llx");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("credentials"), FAKE_KEY).unwrap();
    std::fs::write(config.join("openai.json"), "{\"access_token\": \"secret\"}").unwrap();
    let out = home.run(&["logout"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let shown = text(&out.stdout);
    assert!(shown.contains("OpenRouter: signed out"), "{shown}");
    assert!(
        shown.contains("OpenAI (sign in with ChatGPT): signed out"),
        "{shown}"
    );
    assert!(!shown.contains(FAKE_KEY) && !shown.contains("secret"));
    assert!(!config.join("credentials").exists() && !config.join("openai.json").exists());
    let again = text(&home.run(&["logout"]).stdout);
    assert!(again.contains("OpenRouter: no saved sign-in"), "{again}");
}

#[test]
fn a_provider_not_signed_in_and_a_level_the_model_lacks_stop_before_any_model() {
    let home = Home::new();
    let out = home
        .llx(&["exec", "--provider", "openai", "x"])
        .env("OPENROUTER_API_KEY", FAKE_KEY)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(
        text(&out.stderr).contains("not signed in"),
        "{}",
        text(&out.stderr)
    );
    let out = home
        .llx(&["exec", "--reasoning", "ultra", "x"])
        .env("OPENROUTER_API_KEY", FAKE_KEY)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    let said = text(&out.stderr);
    assert!(
        said.contains("--reasoning ultra: anthropic/claude-haiku-5.5 takes off, low"),
        "{said}"
    );
}

#[test]
fn a_run_without_a_key_stops_before_any_model() {
    let home = Home::new();
    let out = home.run(&["exec", "fix the tests"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(text(&out.stderr).contains("not signed in"));
}

#[test]
fn a_session_without_a_terminal_points_to_exec() {
    let home = Home::new();
    for args in [&["fix the tests"][..], &[], &["--json", "x"]] {
        let out = home
            .llx(args)
            .env("OPENROUTER_API_KEY", FAKE_KEY)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(3), "{args:?}");
        assert!(text(&out.stderr).contains("llx exec"), "{args:?}");
    }
}

#[test]
fn bad_run_arguments_are_usage_errors() {
    let home = Home::new();
    let with_key = |args: &[&str]| {
        home.llx(args)
            .env("OPENROUTER_API_KEY", FAKE_KEY)
            .output()
            .unwrap()
    };
    let missing = home.path().join("missing");
    let out = with_key(&["exec", "-C", missing.to_str().unwrap(), "x"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(text(&out.stderr).contains("-C"));
    let out = with_key(&["exec", "--schema", "[1]", "x"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(text(&out.stderr).contains("--schema"));
    // No prompt, and stdin (closed here) gives none.
    let out = with_key(&["exec"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(text(&out.stderr).contains("no prompt"));
}

#[test]
fn sessions_and_resume_with_none_saved() {
    let home = Home::new();
    let list = home.run(&["sessions"]);
    assert_eq!(list.status.code(), Some(0));
    assert_eq!(text(&list.stdout), "No saved sessions.\n");
    let resume = home
        .llx(&["resume", "--last"])
        .env("OPENROUTER_API_KEY", FAKE_KEY)
        .output()
        .unwrap();
    assert_eq!(resume.status.code(), Some(3));
    assert!(text(&resume.stderr).contains("llx sessions"));
}

/// `llx … | head -1`, where the reader is gone before anything is written.
fn into_a_closed_pipe(command: &mut Command) -> Output {
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    command
        .stdout(writer)
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

#[test]
fn a_closed_stdout_ends_quietly_with_zero() {
    let home = Home::new();
    let help = into_a_closed_pipe(&mut home.llx(&["--help"]));
    assert_eq!(help.status.code(), Some(0));
    let status = into_a_closed_pipe(home.llx(&["logout"]).env("OPENROUTER_API_KEY", FAKE_KEY));
    assert_eq!(status.status.code(), Some(0));
    assert!(status.stderr.is_empty(), "{}", text(&status.stderr));
    let sessions = into_a_closed_pipe(&mut home.llx(&["sessions"]));
    assert_eq!(sessions.status.code(), Some(0));
    assert!(sessions.stderr.is_empty());
}

#[test]
fn doctor_without_a_key_checks_the_rest_then_exits_three() {
    let home = Home::new();
    let out = home.run(&["doctor"]);
    assert_eq!(out.status.code(), Some(3), "{}", text(&out.stderr));
    let shown = text(&out.stdout);
    assert!(shown.contains("shell: "), "{shown}");
    assert!(shown.contains("browser: "), "{shown}");
    assert!(shown.contains("OpenRouter: not signed in"), "{shown}");
    assert!(shown.contains("decision model: "), "{shown}");
}

/// A saved session, as the store writes it.
fn save_session(home: &Home, id: &str, workspace: &str) {
    let dir = home.path().join("sessions");
    std::fs::create_dir_all(&dir).unwrap();
    let session = format!(
        r#"{{"id":"{id}","created":1700000000,"updated":1700000000,"workspace":"{workspace}",
        "turns":[{{"goal":"fix the parser","answer":"Fixed.","status":"achieved","at":1700000000}}]}}"#
    );
    std::fs::write(dir.join(format!("{id}.json")), session).unwrap();
}

#[test]
fn a_saved_session_is_listed_and_resumed() {
    let home = Home::new();
    save_session(&home, "1700000000-ab12", "");
    let list = home.run(&["sessions"]);
    assert_eq!(list.status.code(), Some(0));
    let rows = text(&list.stdout);
    assert!(rows.starts_with("1700000000-ab12  "), "{rows}");
    assert!(rows.contains("fix the parser"), "{rows}");
    let resume = home.run(&["exec", "resume", "1700000000-ab12", "go on"]);
    assert_eq!(resume.status.code(), Some(3));
    assert!(text(&resume.stderr).contains("not signed in"));
}

#[test]
fn resuming_a_session_from_elsewhere_says_where_it_ran() {
    let home = Home::new();
    save_session(&home, "1700000000-cd34", "/elsewhere/repo");
    let resume = home
        .llx(&["resume", "--last"])
        .env("OPENROUTER_API_KEY", FAKE_KEY)
        .output()
        .unwrap();
    // No prompt, and stdin (closed) gives none: it stops before any model.
    assert_eq!(resume.status.code(), Some(3));
    let said = text(&resume.stderr);
    assert!(
        said.contains("session 1700000000-cd34 ran in /elsewhere/repo"),
        "{said}"
    );
}
