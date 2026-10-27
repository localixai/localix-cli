//! One task of an agent, on its own thread: its conversation goes on (or opens with a brief), its
//! answer is posted to the main agent, and the task is recorded in its session.

use std::fmt::Write as _;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use agent::agent::{Agent, Run, Status};
use contracts::chat::{Chat, ChatError, Completion, Request};
use contracts::history::Turn;

use super::Registry;
use super::inbox::AgentInbox;
use super::kit::{Kit, Own, Tools};
use super::watch::{Labeled, Watch};
use crate::mode;
use protocol::Event;

/// What an agent is told before its first task: who it answers to, and how.
fn brief(name: &str) -> String {
    format!(
        "You are `{name}`, an agent another agent started for one task. Your answer goes back to \
         that agent, not to the person, and it is all it gets: make it complete on its own. Say \
         what you found or changed (paths with line numbers), what you checked and how, and what \
         is left. Nobody answers questions here: decide on your best assumption and state it. \
         Do this task only."
    )
}

/// The chat model, stopped when this agent is.
struct Stoppable<'a> {
    inner: &'a dyn Chat,
    stop: &'a (dyn Fn() -> bool + Sync),
}

impl Chat for Stoppable<'_> {
    fn complete(&self, request: Request) -> Result<Completion, ChatError> {
        if (self.stop)() {
            return Err(ChatError::Unavailable("the agent was stopped".into()));
        }
        self.inner.complete(request)
    }
}

/// Give the agent with session `id` the task `message`, and post its answer.
pub(super) fn work(kit: &Kit, registry: &Registry, id: &str, message: &str) {
    let taken = {
        let mut state = registry.lock();
        state
            .children
            .iter_mut()
            .find(|c| c.session.id == id)
            .map(|child| {
                (
                    child.session.name.clone(),
                    child.conversation.take(),
                    std::mem::take(&mut child.seen),
                    child.told.clone(),
                    Arc::clone(&child.stop),
                    Arc::clone(&child.letters),
                )
            })
    };
    let Some((name, conversation, seen, told, flag, letters)) = taken else {
        return;
    };
    let stop = || (kit.stopped)() || flag.load(Ordering::SeqCst);
    let gate = Labeled {
        inner: kit.gate,
        label: name.clone(),
    };
    let own = Own {
        gate: &gate,
        stop: &stop,
        seen: Mutex::new(seen),
    };
    let tools = Tools::new(kit, &own);
    let watch = Watch {
        output: kit.output,
        id,
        name: &name,
    };
    let inbox = AgentInbox {
        letters: &letters,
        watch: &watch,
    };
    let chat = Stoppable {
        inner: kit.chat,
        stop: &stop,
    };
    let now = kit.mode.get();
    watch.show(Event::TaskStarted(message.to_string()));
    let started = Instant::now();
    let run = tools.with(|tools| {
        let agent = Agent {
            chat: &chat,
            tools,
            context: kit.context,
            max_turns: kit.max_turns,
            window: kit.window.load(Ordering::SeqCst),
            strategy: kit.strategy,
            observer: &watch,
            schema: None,
            inbox: Some(&inbox),
        };
        // The brief first, and the mode when it is not the one the context states.
        let mut said = vec![];
        if conversation.is_none() {
            said.push(brief(&name));
        }
        if now != told {
            said.push(mode::notice(&now, &kit.rules, kit.terminal, false));
        }
        let conversation = match said.is_empty() {
            true => conversation,
            false => Some(agent.tell(conversation, &said.join("\n\n"))),
        };
        match conversation {
            Some(conversation) => agent.follow_up(conversation, message),
            None => agent.run(message),
        }
    });
    let halt = stop().then_some(crate::brake::Halt::Interrupted);
    let status = crate::commands::run::Ended::of(&run.status, None, halt, 0).status;
    let millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    watch.show(Event::Answer(run.answer.clone()));
    watch.show(Event::TaskEnded {
        status: status.to_string(),
        millis,
        turns: run.spend.calls,
        cost: run.spend.cost,
    });
    let seen = own
        .seen
        .into_inner()
        .unwrap_or_else(|poison| poison.into_inner());
    keep(kit, registry, id, message, run, status, seen, now);
}

/// What the agent's thread keeps once its record is made, for the work done after the lock.
struct Recorded {
    session: contracts::history::Session,
    unread: Vec<String>,
    name: String,
    open: bool,
}

/// Put the agent's conversation back, record its task, count its spend and post its answer. The
/// registry is locked to change it only: the session is saved and the answer posted after (R6).
#[expect(
    clippy::too_many_arguments,
    reason = "the parts of a finished task, each used once; a struct would only rename them"
)]
fn keep(
    kit: &Kit,
    registry: &Registry,
    id: &str,
    message: &str,
    run: Run,
    status: &str,
    seen: std::collections::HashSet<std::path::PathBuf>,
    told: mode::Mode,
) {
    let recorded = {
        let mut state = registry.lock();
        state.spent.absorb(&run.spend);
        state
            .children
            .iter_mut()
            .find(|c| c.session.id == id)
            .map(|child| {
                child.seen = seen;
                child.told = told;
                child.status = status.to_string();
                if child.open {
                    child.conversation = Some(run.conversation);
                }
                child.session.record(
                    Turn {
                        goal: message.to_string(),
                        answer: run.answer.clone(),
                        status: status.to_string(),
                        at: session::now(),
                        steps: vec![],
                    },
                    None,
                );
                Recorded {
                    session: child.session.clone(),
                    // What the main agent sent after the agent's last look at its letters.
                    unread: child.letters.take(),
                    name: child.session.name.clone(),
                    open: child.open,
                }
            })
    };
    let Some(Recorded {
        session,
        unread,
        name,
        open,
    }) = recorded
    else {
        registry.changed.notify_all();
        return;
    };
    let saved = kit.history.save(&session);
    let mut answer = match &run.status {
        Status::Exhausted => "It ran out of turns before an answer: `send` it a message to have \
                              it answer with what it has."
            .to_string(),
        Status::Error(why) if status != "interrupted" => {
            format!("It stopped: {why}. Its conversation is kept: `send` to go on with it.")
        }
        _ if status == "interrupted" && run.answer.trim().is_empty() => {
            "It was stopped before it answered. Its conversation is kept: `send` to go on with \
             it, or leave it."
                .to_string()
        }
        Status::Achieved | Status::NotAchieved | Status::Unstated | Status::Error(_) => {
            run.answer.trim().to_string()
        }
    };
    if !unread.is_empty() {
        let _ = write!(
            answer,
            "\n\n(it ended before reading: {}; `send` again to have it act on them)",
            unread.join(" | ")
        );
    }
    if let Err(error) = saved {
        let _ = write!(answer, "\n\n(its session was not saved: {error})");
    }
    if open {
        let came = crate::arrivals::report(&name, id, status, &answer);
        registry.queue.post(came);
    }
    // Working ends after the answer is posted: a wait that sees no agent working finds it.
    if let Some(child) = registry
        .lock()
        .children
        .iter_mut()
        .find(|c| c.session.id == id)
    {
        child.working = false;
    }
    registry.changed.notify_all();
}
