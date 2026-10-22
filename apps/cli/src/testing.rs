//! What the app's unit tests share: a fresh directory per test, a writer whose bytes the test
//! reads back, fakes of the models (no network), a person typing at the terminal, and a whole run
//! composed around them.

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::Parser;
use contracts::chat::{Chat, ChatError, Completion, Message, Request};
use contracts::history::History;
use contracts::model::{DecideError, Decider, Decision, Question};
use serde_json::Value;
use tools::workspace::Workspace;

use crate::cli::Cli;
use crate::compose::{Models, Rig, Setup, assemble};
use crate::gate::Prompter;
use crate::interrupt::Interrupt;
use crate::mode::Mode;
use crate::output::Output;

/// A directory for one test, removed when it ends: tests run in parallel, so each gets its own.
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let name = format!(
            "llx-cli-{}-{}-{}",
            std::process::id(),
            session::now(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        // As the workspace names it (on Windows, without the verbatim `\\?\` prefix), so paths
        // compare equal with what the app reports.
        let root = tools::workspace::Workspace::new(&dir).unwrap();
        Self(root.root().to_path_buf())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Write `content` at `name` under the directory, creating its parents.
    pub fn file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A writer that keeps what is written, shared with the test that reads it.
#[derive(Clone, Default)]
pub struct Captured(Rc<RefCell<Vec<u8>>>);

impl Captured {
    pub fn text(&self) -> String {
        String::from_utf8(self.0.borrow().clone()).unwrap()
    }
}

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A writer whose reader has gone: every write is a broken pipe.
pub struct Closed;

impl Write for Closed {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }
}

/// A chat model that answers from a script, then is unreachable.
pub struct ScriptedChat(RefCell<Vec<Completion>>);

impl ScriptedChat {
    pub fn new(replies: Vec<Completion>) -> Self {
        Self(RefCell::new(replies.into_iter().rev().collect()))
    }
}

impl Chat for ScriptedChat {
    fn complete(&self, _: Request) -> Result<Completion, ChatError> {
        self.0
            .borrow_mut()
            .pop()
            .ok_or_else(|| ChatError::Unavailable("the script ran out".into()))
    }
}

/// A decision model that is never there: the tests run without a browser.
pub struct NoDecider;

impl Decider for NoDecider {
    fn decide(&self, _: &Value, _: &BTreeMap<String, Question>) -> Result<Decision, DecideError> {
        Err(DecideError::Unavailable(
            "no decision model in tests".into(),
        ))
    }
}

/// The terminal, with a person who types lines from a script, then nothing (a closed terminal);
/// it remembers each question shown.
#[derive(Default)]
pub struct Typed {
    lines: RefCell<VecDeque<&'static str>>,
    pub asked: RefCell<Vec<String>>,
}

impl Typed {
    pub fn new(lines: &[&'static str]) -> Self {
        Self {
            lines: RefCell::new(lines.iter().copied().collect()),
            asked: RefCell::default(),
        }
    }
}

impl Prompter for &Typed {
    fn ask(&self, question: &str) -> Option<String> {
        self.asked.borrow_mut().push(question.to_string());
        self.lines.borrow_mut().pop_front().map(str::to_string)
    }
}

/// `n` answers that end a task well.
pub fn answers(n: usize) -> Vec<Completion> {
    (0..n)
        .map(|_| Completion {
            text: Some("Done.\nRESULT: achieved".into()),
            ..Completion::default()
        })
        .collect()
}

/// Compose a run that starts in `mode` around a scripted model, in a fresh workspace, and run
/// `work` with it. No terminal: nothing here may wait on a person.
pub fn composed(
    mode: Mode,
    asks: bool,
    replies: Vec<Completion>,
    work: impl FnOnce(&mut Rig) -> i32,
) {
    let scratch = Scratch::new();
    let history = session::Sessions {
        dir: scratch.path().join("sessions"),
    };
    let session = history.start();
    let setup = Setup {
        options: Cli::try_parse_from(["llx"]).unwrap().run,
        choice: crate::choice::Choice::new(crate::providers::Provider::OpenRouter, "test/model"),
        workspace: Workspace::new(scratch.path()).unwrap(),
        context: String::new(),
        fresh: String::new(),
        schema: None,
        mode,
        terminal: false,
        asks,
        history,
        session,
    };
    let chat = ScriptedChat::new(replies);
    let models = Models {
        chat: &chat,
        decider: &NoDecider,
        live: None,
    };
    let output = Output::with(
        false,
        false,
        Box::new(Captured::default()),
        Box::new(Captured::default()),
    );
    assemble(setup, models, &output, &Interrupt::new(), None, work);
}

/// The user messages of the rig's conversation: the goals and the notes before them.
pub fn users(rig: &Rig) -> Vec<String> {
    let conversation = rig.conversation.as_ref().unwrap();
    conversation
        .messages()
        .iter()
        .filter_map(|message| match message {
            Message::User(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}
