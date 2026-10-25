//! The mode a run works in: a name, a line on what it does, and its rules (ARCHITECTURE.md K5).
//! The gate judges each effect by the mode's rules and the command line's, then asks the person
//! what no rule settles, or refuses it with no one to ask. The context says the same rules, from
//! the same values, so the two cannot drift apart; a mode never filters the tools (the list stays
//! the same, and so does the prompt cache). A change in a session reaches the model as a
//! `<mode_change>` note.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::gate::{Effect, Gate, Verdict};

use crate::gate::{Asker, Prompter, TerminalGate, declined};
use crate::output::lock;
use crate::rules::{Rules, Ruling};
use protocol::Approval;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mode(Arc<Named>);

#[derive(Debug, PartialEq, Eq)]
struct Named {
    name: String,
    hint: String,
    rules: Rules,
}

impl Mode {
    pub(crate) fn new(name: &str, hint: &str, rules: Rules) -> Self {
        Self(Arc::new(Named {
            name: name.to_string(),
            hint: hint.to_string(),
            rules,
        }))
    }

    pub(crate) fn name(&self) -> &str {
        &self.0.name
    }

    /// One line on what the mode does.
    pub(crate) fn hint(&self) -> &str {
        &self.0.hint
    }

    pub(crate) fn rules(&self) -> &Rules {
        &self.0.rules
    }
}

/// The mode now, read by the gate and the person on whichever thread asks (an agent's, a call's
/// running beside others) and set between tasks.
#[derive(Debug)]
pub(crate) struct ModeCell(Mutex<Mode>);

impl ModeCell {
    pub(crate) fn new(mode: Mode) -> Self {
        Self(Mutex::new(mode))
    }

    pub(crate) fn get(&self) -> Mode {
        lock(&self.0).clone()
    }

    pub(crate) fn set(&self, mode: Mode) {
        *lock(&self.0) = mode;
    }
}

/// `/mode`: each mode with what it does, the current one marked.
pub(crate) fn list(current: &Mode, modes: &[Mode]) -> String {
    let line = |mode: &Mode| {
        let mark = if mode.name() == current.name() {
            "*"
        } else {
            " "
        };
        format!("{mark} {:<11} {}\n", mode.name(), mode.hint())
    };
    modes.iter().map(line).collect()
}

/// What the person is told when the mode changes.
pub(crate) fn switched(mode: &Mode) -> String {
    format!("Mode: {}. {}\n", mode.name(), mode.hint())
}

/// Why an effect no rule allows is refused with no one to approve it.
const NO_ONE: &str = "no rule allows it, and no one is at a terminal to approve it: the person \
can rerun llx with --allow and a rule that names it, a --mode that allows it, or --yes (-y)";

/// Why an effect is refused when the terminal gave no answer.
const NO_ANSWER: &str = "no answer came from the terminal";

/// The mode as the agent's context states it, with `more` (the command line's rules) beside its
/// own. `terminal`: someone can approve at the terminal. `asks`: the `ask` tool is offered.
pub(crate) fn describe(mode: &Mode, more: &Rules, terminal: bool, asks: bool) -> String {
    let mut text = format!("{}: {}", mode.name(), mode.hint());
    if let Some(rules) = mode.rules().with(more).describe() {
        text.push(' ');
        text.push_str(&rules);
    }
    text.push_str(match terminal {
        true => " Anything else is asked of the person, who may refuse it; a refusal says why.",
        false => {
            " Anything else that changes something (a command, a file, a page) is refused: no one \
             is at a terminal to approve it. Reading files works."
        }
    });
    if asks && terminal {
        text.push_str(
            " Ask the person with `ask` only when a decision is theirs and the tools cannot \
             settle it; otherwise make a reasonable assumption and state it.",
        );
    }
    text
}

/// The note that tells the model the mode changed mid-session.
pub(crate) fn notice(mode: &Mode, more: &Rules, terminal: bool, asks: bool) -> String {
    format!(
        "<mode_change>\nThe mode is now {}\nWhat the context or an earlier note said about the \
         mode no longer holds.\n</mode_change>",
        describe(mode, more, terminal, asks)
    )
}

/// The gate as the current mode answers it: its rules and the command line's first, then the
/// person. Each mode keeps its own "always" answers, so one given in a mode that asks little does
/// not carry into one meant to ask more.
pub(crate) struct ModeGate<'a> {
    mode: &'a ModeCell,
    /// `None`: nobody to ask.
    asker: Option<Box<dyn Asker + 'a>>,
    /// The tools the person said not to ask about again, by mode.
    always: Mutex<HashMap<String, HashSet<String>>>,
    /// Told how long the person took to answer, so it does not count toward the time limit.
    waited: &'a (dyn Fn(Duration) + Sync),
    /// `--allow` and `--deny`, beside the mode's own.
    rules: Rules,
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
            always: Mutex::default(),
            waited,
            rules: Rules::default(),
        }
    }

    /// The same, with the command line's `rules` beside the mode's.
    pub(crate) fn ruled(self, rules: Rules) -> Self {
        Self { rules, ..self }
    }
}

impl Gate for ModeGate<'_> {
    fn check(&self, effect: Effect) -> Verdict {
        let mode = self.mode.get();
        match mode
            .rules()
            .with(&self.rules)
            .judge(effect.tool, effect.subject)
        {
            Ruling::Denied(why) => return Verdict::Deny(format!("{why} (mode {})", mode.name())),
            Ruling::Allowed => return Verdict::Allow,
            Ruling::Unsettled => {}
        }
        let Some(asker) = &self.asker else {
            return Verdict::Deny(NO_ONE.into());
        };
        let known = |always: &HashMap<String, HashSet<String>>| {
            always
                .get(mode.name())
                .is_some_and(|tools| tools.contains(effect.tool))
        };
        if known(&lock(&self.always)) {
            return Verdict::Allow;
        }
        // The asker holds its person for the question (`gate::one_question`).
        let asked = Instant::now();
        let approval = asker.approve(&effect);
        (self.waited)(asked.elapsed());
        match approval {
            Some(Approval::Yes) => Verdict::Allow,
            Some(Approval::Always) => {
                let mut always = lock(&self.always);
                let tools = always.entry(mode.name().to_string()).or_default();
                tools.insert(effect.tool.to_string());
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
