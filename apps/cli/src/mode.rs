//! The mode a run works in: one setting for what runs without asking, what is refused, and whether
//! the person answers questions. A mode changes only what the gate and the person answer and what
//! the context's `Mode:` line says, from the same value, so the two cannot drift apart; it never
//! filters the tools (the list stays the same, and so does the prompt cache). A change in a
//! session reaches the model as a `<mode_change>` note.

use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use clap::ValueEnum;
use contracts::gate::{Effect, Gate, Verdict};

use crate::gate::{Asker, Prompter, TerminalGate, declined};
use crate::output::lock;
use crate::tui::event::Approval;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Mode {
    /// Each command, file change and browser action is asked at the terminal.
    Default,
    /// Investigate and plan: file changes are refused, each command and browser action is asked.
    Plan,
    /// Everything runs without asking (-y); questions still reach you.
    Bypass,
    /// Everything runs without asking, and no question is asked (in exec, the same as bypass).
    Autonomous,
}

impl Mode {
    pub(crate) const ALL: [Self; 4] = [Self::Default, Self::Plan, Self::Bypass, Self::Autonomous];

    /// `-y` is bypass; otherwise the flag, else default.
    pub(crate) fn choose(flag: Option<Self>, yes: bool) -> Self {
        match yes {
            true => Self::Bypass,
            false => flag.unwrap_or(Self::Default),
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Plan => "plan",
            Self::Bypass => "bypass",
            Self::Autonomous => "autonomous",
        }
    }

    /// The mode called `name`, as `--mode` spells it.
    pub(crate) fn named(name: &str) -> Option<Self> {
        Self::from_str(name.trim(), true).ok()
    }

    /// One line on what the mode does: its `--help` text.
    pub(crate) fn hint(self) -> String {
        self.to_possible_value()
            .and_then(|value| value.get_help().map(ToString::to_string))
            .unwrap_or_default()
    }
}

/// The mode now, read by the gate and the person on whichever thread asks (an agent's, a call's
/// running beside others) and set between tasks.
#[derive(Debug)]
pub(crate) struct ModeCell(AtomicU8);

impl ModeCell {
    pub(crate) fn new(mode: Mode) -> Self {
        Self(AtomicU8::new(Self::index(mode)))
    }

    pub(crate) fn get(&self) -> Mode {
        Mode::ALL[self.0.load(Ordering::SeqCst) as usize]
    }

    pub(crate) fn set(&self, mode: Mode) {
        self.0.store(Self::index(mode), Ordering::SeqCst);
    }

    fn index(mode: Mode) -> u8 {
        let at = Mode::ALL.iter().position(|&m| m == mode);
        at.and_then(|at| u8::try_from(at).ok()).unwrap_or(0)
    }
}

/// `/mode`: each mode with what it does, the current one marked.
pub(crate) fn list(current: Mode) -> String {
    let line = |mode: Mode| {
        let mark = if mode == current { "*" } else { " " };
        format!("{mark} {:<11} {}\n", mode.name(), mode.hint())
    };
    Mode::ALL.map(line).concat()
}

/// What the person is told when the mode changes.
pub(crate) fn switched(mode: Mode) -> String {
    format!("Mode: {}. {}\n", mode.name(), mode.hint())
}

/// Why plan mode refuses a file change.
pub(crate) const PLAN_REFUSAL: &str = "plan mode is read-only: files are not changed in it. Investigate \
with reads and commands that change nothing, then answer with the plan; the person switches the \
mode to have it carried out.";

/// Why an effect is refused with no one to approve it.
pub(crate) const NO_TERMINAL: &str = "no one is at a terminal to approve it; the person must rerun \
llx with --yes (-y) or --mode bypass to allow commands, file changes and browser actions";

/// Why plan mode refuses a command or a browser action with no one to approve it. It never points
/// to bypass: that would turn a read-only request into one that changes files unasked.
pub(crate) const PLAN_NO_TERMINAL: &str = "no one is at a terminal to approve it, and plan mode runs \
nothing unapproved: plan from reading files, and say in the plan which commands to run, or that \
the person can run llx at a terminal to approve each one";

/// Why an effect is refused when the terminal gave no answer.
const NO_ANSWER: &str = "no answer came from the terminal";

/// Why a question gets no answer in autonomous mode.
pub(crate) const AUTONOMOUS: &str = "nobody answers questions in autonomous mode: decide on your best \
assumption, state it in your answer, and keep going";

/// The tool plan mode refuses outright: the one that changes files.
const EDIT: &str = "edit";

/// The mode as the agent's context states it. `terminal`: someone can approve at the terminal.
/// `asks`: the `ask` tool is offered (a session).
pub(crate) fn describe(mode: Mode, terminal: bool, asks: bool) -> String {
    let asks = asks && terminal;
    let mut text = String::new();
    match mode {
        Mode::Default if terminal => {
            text.push_str(
                "default: the person approves each command, file change and browser action at \
                 the terminal; a refusal says why.",
            );
            if asks {
                text.push_str(
                    " Ask the person with `ask` only when a decision is theirs and the tools \
                     cannot settle it; otherwise make a reasonable assumption and state it.",
                );
            }
        }
        Mode::Default => text.push_str(
            "default: commands, file changes and browser actions are refused: no one is at a \
             terminal to approve them, and the person must rerun with --yes (-y) or --mode bypass \
             to allow them; reading files works.",
        ),
        Mode::Plan => {
            text.push_str(
                "plan: investigate and plan; do not do the work. `edit` is refused in this mode",
            );
            match terminal {
                true => text.push_str(
                    ". Use commands and the browser only to look (read, search, list, run checks \
                     that change no tracked file); the person approves each one and refuses any \
                     that would change something.",
                ),
                false => text.push_str(
                    ", and commands and browser actions are refused too (no one at a terminal to \
                     approve them): plan from reading files.",
                ),
            }
            if asks {
                text.push_str(
                    " Ask about preferences and tradeoffs with `ask`, after you have looked.",
                );
            }
            text.push_str(
                " When the plan is complete enough that whoever carries it out makes no decision, \
                 answer with it: a title, a short summary, the changes file by file, the tests to \
                 add or run, and the assumptions you made. Do not ask whether to proceed: the \
                 person switches the mode to have it carried out.",
            );
        }
        Mode::Bypass => {
            text.push_str("bypass: commands, file changes and browser actions run without asking.");
            if asks {
                text.push_str(
                    " The person can still answer questions with `ask`; use it only when a \
                     decision is theirs.",
                );
            }
        }
        Mode::Autonomous => text.push_str(
            "autonomous: commands, file changes and browser actions run without asking, and \
             nobody answers questions: decide on your best assumption, state it in your answer, \
             and keep going.",
        ),
    }
    text
}

/// The note that tells the model the mode changed mid-session.
pub(crate) fn notice(mode: Mode, terminal: bool, asks: bool) -> String {
    format!(
        "<mode_change>\nThe mode is now {}\nWhat the context or an earlier note said about the \
         mode no longer holds.\n</mode_change>",
        describe(mode, terminal, asks)
    )
}

/// The gate as the current mode answers it: bypass and autonomous allow, plan refuses file changes
/// and asks for the rest, default asks. Plan keeps its own "always" answers, so an `a` given while
/// working does not let a command through unasked while planning, nor the reverse.
pub(crate) struct ModeGate<'a> {
    mode: &'a ModeCell,
    /// `None`: nobody to ask.
    asker: Option<Box<dyn Asker + 'a>>,
    /// The tools the person said not to ask about again: while working, and while planning.
    always: [Mutex<HashSet<String>>; 2],
    /// Told how long the person took to answer, so it does not count toward the time limit.
    waited: &'a (dyn Fn(Duration) + Sync),
}

impl<'a> ModeGate<'a> {
    /// Asking on `terminal`, when there is one.
    pub(crate) fn new<P: Prompter + 'a>(
        mode: &'a ModeCell,
        terminal: Option<P>,
        waited: &'a (dyn Fn(Duration) + Sync),
    ) -> Self {
        let asker = terminal.map(|p| Box::new(TerminalGate(p)) as Box<dyn Asker + 'a>);
        Self::asking(mode, asker, waited)
    }

    /// Asking `asker`, when there is one.
    pub(crate) fn asking(
        mode: &'a ModeCell,
        asker: Option<Box<dyn Asker + 'a>>,
        waited: &'a (dyn Fn(Duration) + Sync),
    ) -> Self {
        Self {
            mode,
            asker,
            always: Default::default(),
            waited,
        }
    }
}

impl Gate for ModeGate<'_> {
    fn check(&self, effect: Effect) -> Verdict {
        let (always, none) = match self.mode.get() {
            Mode::Bypass | Mode::Autonomous => return Verdict::Allow,
            Mode::Plan if effect.tool == EDIT => return Verdict::Deny(PLAN_REFUSAL.into()),
            Mode::Plan => (&self.always[1], PLAN_NO_TERMINAL),
            Mode::Default => (&self.always[0], NO_TERMINAL),
        };
        let Some(asker) = &self.asker else {
            return Verdict::Deny(none.into());
        };
        if lock(always).contains(effect.tool) {
            return Verdict::Allow;
        }
        let asked = Instant::now();
        let _asking = crate::gate::one_question();
        let approval = asker.approve(&effect);
        (self.waited)(asked.elapsed());
        match approval {
            Some(Approval::Yes) => Verdict::Allow,
            Some(Approval::Always) => {
                lock(always).insert(effect.tool.to_string());
                Verdict::Allow
            }
            Some(Approval::No(why)) => Verdict::Deny(declined(why)),
            None => Verdict::Deny(NO_ANSWER.into()),
        }
    }
}

#[cfg(test)]
#[path = "mode_tests.rs"]
mod tests;
