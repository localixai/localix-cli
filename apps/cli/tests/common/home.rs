//! A home of its own for each test, and `llx` run in it: no key in the environment, stdin closed,
//! nothing of the person's read.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::model::Model;

/// An OpenRouter key's shape, never a real one.
pub(crate) const FAKE_KEY: &str = "sk-or-v1-0000000000000000000000000000wxyz";

/// A fresh, empty home for one test, removed when it ends.
pub(crate) struct Home(PathBuf);

impl Home {
    pub(crate) fn new() -> Self {
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

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// `llx args…` with this home, no key in the environment and stdin closed.
    pub(crate) fn llx(&self, args: &[&str]) -> Command {
        let mut command = self.command(env!("CARGO_BIN_EXE_llx"));
        command.args(args);
        command
    }

    /// `program` as [`Home::llx`] runs the binary: in this home, stdin closed.
    pub(crate) fn command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        isolate(&mut command, &self.0);
        command.stdin(Stdio::null());
        command
    }

    /// `llx args…` run to its end.
    pub(crate) fn run(&self, args: &[&str]) -> Output {
        self.llx(args).output().unwrap()
    }

    /// `llx args…` run to its end with [`FAKE_KEY`] in the environment.
    pub(crate) fn run_with_key(&self, args: &[&str]) -> Output {
        self.llx(args)
            .env("OPENROUTER_API_KEY", FAKE_KEY)
            .output()
            .unwrap()
    }

    /// `llx args…` signed in with [`FAKE_KEY`], thinking with the scripted `model`.
    pub(crate) fn with_model(&self, model: &Model, args: &[&str]) -> Command {
        let mut command = self.llx(args);
        command
            .env("OPENROUTER_API_KEY", FAKE_KEY)
            .env("OPENROUTER_BASE_URL", &model.base);
        command
    }

    /// A saved session, as the store writes it, last updated at `updated`: one turn, "fix the
    /// parser".
    pub(crate) fn save_session(&self, id: &str, workspace: &str, updated: u64) {
        let dir = self.0.join("sessions");
        std::fs::create_dir_all(&dir).unwrap();
        let session = format!(
            r#"{{"id":"{id}","created":1700000000,"updated":{updated},"workspace":"{workspace}",
            "turns":[{{"goal":"fix the parser","answer":"Fixed.","status":"achieved","at":1700000000}}]}}"#
        );
        std::fs::write(dir.join(format!("{id}.json")), session).unwrap();
    }

    /// The saved session `id`, as JSON.
    pub(crate) fn session(&self, id: &str) -> serde_json::Value {
        let file = self.0.join("sessions").join(format!("{id}.json"));
        serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The environment every run of the binary gets, and nothing else of the shell that runs the
/// tests: no provider key, no setting, and a model address that fails at once, so a test never
/// depends on the machine and never reaches a real model.
pub(crate) fn isolate(command: &mut Command, home: &Path) {
    command.env_clear();
    // What the system itself needs to start a process and find a temporary directory, and where
    // an instrumented binary writes its coverage.
    for name in [
        "PATH",
        "TMPDIR",
        "TEMP",
        "TMP",
        "SystemRoot",
        "LLVM_PROFILE_FILE",
    ] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("APPDATA", home.join("config"))
        .env("LLX_SESSIONS", home.join("sessions"))
        .env("LLX_MODELS_REFRESH", "0")
        .env("LLX_XTERM", "0")
        .env("OPENROUTER_BASE_URL", "http://127.0.0.1:1");
}

/// Bytes a process wrote, as text.
pub(crate) fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
