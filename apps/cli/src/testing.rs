//! What the app's unit tests share: a fresh directory per test, a writer whose bytes the test
//! reads back, fakes of the models (no network), a person typing at the terminal, and a whole run
//! composed around them.

use std::collections::{BTreeMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

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
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Self {
        Self::under(&std::env::temp_dir())
    }

    /// A directory in no git repository, where one can be found: the system's temporary
    /// directory may sit inside one (TMPDIR=$PWD/tmp, a devcontainer), so the usual others are
    /// tried too.
    pub(crate) fn outside_repository() -> Option<Self> {
        let mut bases = vec![std::env::temp_dir()];
        if cfg!(unix) {
            bases.extend(["/tmp", "/var/tmp"].map(PathBuf::from));
        }
        bases.into_iter().find_map(|base| {
            let scratch = Self::under(&base);
            let in_repository = scratch
                .path()
                .ancestors()
                .any(|dir| dir.join(".git").exists());
            (!in_repository).then_some(scratch)
        })
    }

    fn under(base: &Path) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let name = format!(
            "llx-cli-{}-{}-{}",
            std::process::id(),
            session::now(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let dir = base.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        // As the workspace names it (on Windows, without the verbatim `\\?\` prefix), so paths
        // compare equal with what the app reports.
        let root = Workspace::new(&dir).unwrap();
        Self(root.root().to_path_buf())
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// Write `content` at `name` under the directory, creating its parents.
    pub(crate) fn file(&self, name: &str, content: &str) -> PathBuf {
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
pub(crate) struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    pub(crate) fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

impl Write for Captured {
    #[expect(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A writer whose reader has gone: every write is a broken pipe.
pub(crate) struct Closed;

impl Write for Closed {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }
}

/// A chat model that answers from a script, then is unreachable.
pub(crate) struct ScriptedChat(Mutex<Vec<Completion>>);

impl ScriptedChat {
    pub(crate) fn new(replies: Vec<Completion>) -> Self {
        Self(Mutex::new(replies.into_iter().rev().collect()))
    }
}

impl Chat for ScriptedChat {
    #[expect(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn complete(&self, _: Request) -> Result<Completion, ChatError> {
        self.0
            .lock()
            .unwrap()
            .pop()
            .ok_or_else(|| ChatError::Unavailable("the script ran out".into()))
    }
}

/// A decision model that is never there: the tests run without a browser.
pub(crate) struct NoDecider;

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
pub(crate) struct Typed {
    lines: Mutex<VecDeque<&'static str>>,
    pub asked: Mutex<Vec<String>>,
}

impl Typed {
    pub(crate) fn new(lines: &[&'static str]) -> Self {
        Self {
            lines: Mutex::new(lines.iter().copied().collect()),
            asked: Mutex::default(),
        }
    }
}

impl Prompter for &Typed {
    fn ask(&self, question: &str) -> Option<String> {
        self.asked.lock().unwrap().push(question.to_string());
        self.lines.lock().unwrap().pop_front().map(str::to_string)
    }
}

/// `n` answers that end a task well.
pub(crate) fn answers(n: usize) -> Vec<Completion> {
    (0..n)
        .map(|_| Completion {
            text: Some("Done.\nRESULT: achieved".into()),
            ..Completion::default()
        })
        .collect()
}

/// Compose a run that starts in `mode` around a scripted model, in a fresh workspace, and run
/// `work` with it. No terminal: nothing here may wait on a person.
pub(crate) fn composed(
    mode: Mode,
    asks: bool,
    replies: Vec<Completion>,
    work: impl FnOnce(&mut Rig) -> i32,
) {
    composed_with(mode, asks, &ScriptedChat::new(replies), work);
}

/// [`composed`], around `chat`.
pub(crate) fn composed_with(
    mode: Mode,
    asks: bool,
    chat: &dyn Chat,
    work: impl FnOnce(&mut Rig) -> i32,
) {
    let scratch = Scratch::new();
    let (shared, seat) = setup(&scratch, mode, asks).split(None);
    let models = Models {
        chat,
        decider: &NoDecider,
        live: None,
    };
    assemble(&shared, seat, models, &quiet(), &Interrupt::new(), work);
}

/// A session in `scratch` that starts in `mode`, with no terminal and no interface; its
/// sessions are kept in `scratch` too.
pub(crate) fn setup(scratch: &Scratch, mode: Mode, asks: bool) -> Setup {
    let history = session::Sessions {
        dir: scratch.path().join("sessions"),
    };
    let session = history.start();
    Setup {
        options: Cli::try_parse_from(["llx"]).unwrap().run,
        choice: crate::choice::Choice::new(crate::providers::Provider::openrouter(), "test/model"),
        workspace: Workspace::new(scratch.path()).unwrap(),
        context: String::new(),
        fresh: String::new(),
        schema: None,
        mode,
        modes: crate::modes::Modes::built_in(),
        rules: crate::rules::Rules::default(),
        terminal: false,
        asks,
        history,
        session,
        ui: None,
        shown: false,
        #[cfg(unix)]
        theme: None,
    }
}

/// Output nobody reads.
pub(crate) fn quiet() -> Output {
    Output::with(
        false,
        false,
        Box::new(Captured::default()),
        Box::new(Captured::default()),
    )
}

/// An answer that says `text` and ends the task well.
pub(crate) fn says(text: &str) -> Completion {
    Completion {
        text: Some(format!("{text}\nRESULT: achieved")),
        ..Completion::default()
    }
}

/// What the main conversation's calls came back with, in order: whether each failed, and its
/// report.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
pub(crate) fn results(rig: &Rig) -> Vec<(bool, String)> {
    let conversation = rig.conversation.as_ref().unwrap();
    conversation
        .messages()
        .iter()
        .filter_map(|message| match message {
            Message::ToolResult {
                content, failed, ..
            } => Some((*failed, content.clone())),
            _ => None,
        })
        .collect()
}

/// The user messages of the rig's conversation: the goals and the notes before them.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
pub(crate) fn users(rig: &Rig) -> Vec<String> {
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

/// A person who picks from a script, and remembers what each step offered.
#[derive(Default)]
pub(crate) struct Scripted {
    picks: VecDeque<Option<usize>>,
    pub shown: Vec<(String, Vec<crate::picker::Item>)>,
}

impl Scripted {
    pub(crate) fn new(picks: &[Option<usize>]) -> Self {
        Self {
            picks: picks.iter().copied().collect(),
            shown: vec![],
        }
    }

    pub(crate) fn titles(&self) -> Vec<&str> {
        self.shown.iter().map(|(title, _)| title.as_str()).collect()
    }

    pub(crate) fn offered(&self, title: &str) -> Vec<(String, bool)> {
        let (_, items) = self
            .shown
            .iter()
            .find(|(shown, _)| shown.starts_with(title))
            .unwrap();
        items
            .iter()
            .map(|item| (item.label.clone(), item.marked))
            .collect()
    }
}

impl crate::picker::Picker for Scripted {
    fn pick(&mut self, title: &str, items: &[crate::picker::Item]) -> Option<usize> {
        self.shown.push((title.to_string(), items.to_vec()));
        let Some(pick) = self.picks.pop_front() else {
            panic!("no pick was scripted")
        };
        pick
    }
}

impl crate::picker::Lines for Scripted {
    fn line(&mut self, _: &str, _: bool) -> Option<String> {
        None
    }
}

/// A chat model the test answers one turn at a time (a turn waits until it is sent its reply),
/// and the conversations it was asked to go on from.
pub(crate) struct Gated {
    replies: Mutex<std::sync::mpsc::Receiver<Completion>>,
    pub asked: Mutex<Vec<Vec<Message>>>,
}

/// A [`Gated`] model, and where its replies are sent.
pub(crate) fn gated() -> (std::sync::mpsc::Sender<Completion>, Gated) {
    #[expect(
        clippy::disallowed_methods,
        reason = "a test's script: it sends the turns it means the model to take, then ends"
    )]
    let (replies, received) = std::sync::mpsc::channel();
    let chat = Gated {
        replies: Mutex::new(received),
        asked: Mutex::default(),
    };
    (replies, chat)
}

impl Gated {
    /// How many turns it was asked for.
    pub(crate) fn turns(&self) -> usize {
        self.asked.lock().unwrap().len()
    }
}

impl Chat for Gated {
    #[expect(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        self.asked
            .lock()
            .unwrap()
            .push(request.conversation.to_vec());
        let replies = self.replies.lock().unwrap();
        // A test that went wrong ends instead of waiting for ever.
        let waited = replies.recv_timeout(std::time::Duration::from_secs(20));
        waited.map_err(|error| ChatError::Unavailable(format!("the test sent no reply: {error}")))
    }
}

/// The sessions' side of an interface no one draws: the test reads the frames it is sent.
pub(crate) fn fake_ui() -> (
    crate::server::Ui,
    std::sync::mpsc::Receiver<protocol::ToClient>,
) {
    let (ui, _peer, frames) = fake_peer();
    (ui, frames)
}

/// [`fake_ui`], and the server's side of the interface: replies and entries go through it.
pub(crate) fn fake_peer() -> (
    crate::server::Ui,
    Arc<crate::server::Peer>,
    std::sync::mpsc::Receiver<protocol::ToClient>,
) {
    #[expect(
        clippy::disallowed_methods,
        reason = "the in-process transport's channel, as the binary makes it"
    )]
    let (out, frames) = std::sync::mpsc::channel();
    let peer = crate::server::Peer::new(out);
    (crate::server::Ui::new(Arc::clone(&peer)), peer, frames)
}

/// An interface whose person answers each question with `answer` (nothing for `None`), on a
/// thread of its own until the interface is gone; what was asked, in order, and in which session.
pub(crate) fn answering(
    answer: impl Fn(&protocol::Asked) -> Option<protocol::Reply> + Send + 'static,
) -> (
    crate::server::Ui,
    std::thread::JoinHandle<Vec<(String, protocol::Asked)>>,
) {
    let (ui, peer, frames) = fake_peer();
    // Held weakly: the frames end once the test lets the interface go.
    let peer = Arc::downgrade(&peer);
    let person = std::thread::Builder::new().name("person".into());
    let person = person.spawn(move || {
        let mut asked = vec![];
        for frame in frames {
            if let protocol::ToClient::Ask {
                id,
                session,
                asked: question,
            } = frame
            {
                let reply = answer(&question);
                asked.push((session, question));
                if let (Some(reply), Some(peer)) = (reply, peer.upgrade()) {
                    peer.replied(id, reply);
                }
            }
        }
        asked
    });
    (ui, person.unwrap())
}

/// The events sent so far, each with the session it is in; questions and the rest are left out.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
pub(crate) fn events(
    frames: &std::sync::mpsc::Receiver<protocol::ToClient>,
) -> Vec<(Option<String>, protocol::Event)> {
    let events = frames.try_iter().filter_map(|frame| match frame {
        protocol::ToClient::Event { session, event, .. } => Some((session, event)),
        _ => None,
    });
    events.collect()
}
