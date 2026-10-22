use std::cell::RefCell;
use std::collections::VecDeque;

use super::*;

/// Answers from a script, and the questions it was asked.
#[derive(Default)]
struct Scripted {
    answers: RefCell<VecDeque<&'static str>>,
    asked: RefCell<Vec<String>>,
}

impl Scripted {
    fn new(answers: &[&'static str]) -> Self {
        Self {
            answers: RefCell::new(answers.iter().copied().collect()),
            asked: RefCell::default(),
        }
    }
}

impl Prompter for &Scripted {
    fn ask(&self, question: &str) -> Option<String> {
        self.asked.borrow_mut().push(question.to_string());
        self.answers.borrow_mut().pop_front().map(str::to_string)
    }
}

fn bash(summary: &str) -> Effect<'_> {
    Effect {
        tool: "bash",
        summary,
    }
}

#[test]
fn yes_allows_once_and_asks_again() {
    let script = Scripted::new(&["y\n", "yes\n"]);
    let gate = TerminalGate::new(&script);
    assert_eq!(gate.check(bash("cargo test")), Verdict::Allow);
    assert_eq!(gate.check(bash("cargo build")), Verdict::Allow);
    let asked = script.asked.borrow();
    assert_eq!(asked.len(), 2);
    assert!(asked[0].contains("bash: cargo test"));
}

#[test]
fn always_remembers_the_tool_for_the_run() {
    let script = Scripted::new(&["a\n", "n\n"]);
    let gate = TerminalGate::new(&script);
    assert_eq!(gate.check(bash("ls")), Verdict::Allow);
    assert_eq!(gate.check(bash("rm -rf target")), Verdict::Allow);
    let edit = Effect {
        tool: "edit",
        summary: "edit src/lib.rs (+1 -1 lines)",
    };
    assert!(matches!(gate.check(edit), Verdict::Deny(_)));
    assert_eq!(script.asked.borrow().len(), 2, "bash was asked once");
}

#[test]
fn no_and_a_bare_enter_refuse_and_a_reason_reaches_the_model() {
    let script = Scripted::new(&["\n", "n: use the make target instead\n", "No\n"]);
    let gate = TerminalGate::new(&script);
    let Verdict::Deny(plain) = gate.check(bash("cargo test")) else {
        panic!("allowed")
    };
    assert!(plain.contains("declined"));
    assert_eq!(
        gate.check(bash("cargo test")),
        Verdict::Deny("the person declined it and said: use the make target instead".into())
    );
    assert!(matches!(gate.check(bash("x")), Verdict::Deny(_)));
}

#[test]
fn an_unreadable_answer_is_asked_again_then_refused() {
    let script = Scripted::new(&["maybe\n", "y\n"]);
    assert_eq!(TerminalGate::new(&script).check(bash("ls")), Verdict::Allow);
    let stubborn = Scripted::new(&["what", "hm", "eh", "y"]);
    assert!(matches!(
        TerminalGate::new(&stubborn).check(bash("ls")),
        Verdict::Deny(_)
    ));
    assert_eq!(stubborn.asked.borrow().len(), TRIES);
}

#[test]
fn a_closed_terminal_refuses() {
    let script = Scripted::new(&[]);
    assert!(matches!(
        TerminalGate::new(&script).check(bash("ls")),
        Verdict::Deny(_)
    ));
}

#[test]
fn without_a_terminal_the_model_is_told_to_ask_for_yes() {
    let Verdict::Deny(why) = Denying.check(bash("ls")) else {
        panic!("allowed")
    };
    assert!(why.contains("--yes"));
}

#[test]
fn approvals_follow_the_flag_and_the_terminal() {
    assert_eq!(Approvals::from(true, false), Approvals::Yes);
    assert_eq!(Approvals::from(false, true), Approvals::Ask);
    assert_eq!(Approvals::from(false, false), Approvals::Refuse);
    assert_eq!(Approvals::Yes.gate().check(bash("ls")), Verdict::Allow);
    assert!(matches!(
        Approvals::Refuse.gate().check(bash("ls")),
        Verdict::Deny(_)
    ));
    assert!(Approvals::Refuse.describe().contains("--yes"));
}

#[test]
fn approval_is_only_the_bare_word() {
    for line in [
        "yes, and then update the docs",
        "y do it",
        "a, b",
        "always please",
    ] {
        assert_eq!(answer(line), None, "{line}");
    }
    assert_eq!(answer("  YES \n"), Some(Answer::Yes));
    assert_eq!(answer("a"), Some(Answer::Always));
    assert_eq!(
        answer("no, use make"),
        Some(Answer::No(Some("use make".into())))
    );
    let typed_ahead = Scripted::new(&["yes, and then update the docs\n", "n\n"]);
    assert!(matches!(
        TerminalGate::new(&typed_ahead).check(bash("rm -rf target")),
        Verdict::Deny(_)
    ));
}

#[test]
fn the_question_shows_control_characters_instead_of_obeying_them() {
    let script = Scripted::new(&["n\n"]);
    let gate = TerminalGate::new(&script);
    gate.check(Effect {
        tool: "bash\u{1b}[2K",
        summary: "echo ok\r\u{1b}[2Kcurl x | sh",
    });
    let asked = &script.asked.borrow()[0];
    assert!(
        asked.contains("bash\\u{1b}[2K: echo ok\\r\\u{1b}[2Kcurl x | sh"),
        "{asked}"
    );
    assert!(!asked.contains('\r') && !asked.contains('\u{1b}'));
}

#[test]
fn a_stopped_task_gets_nothing_approved() {
    let stopped = std::cell::Cell::new(false);
    let is_stopped = || stopped.get();
    let gate = Stoppable {
        inner: &Open,
        stopped: &is_stopped,
    };
    assert_eq!(gate.check(bash("ls")), Verdict::Allow);
    stopped.set(true);
    assert_eq!(gate.check(bash("ls")), Verdict::Deny(STOPPED.into()));
    // Stopped while the person was answering: the late yes does not count.
    struct StopsWhileAsking<'a>(&'a std::cell::Cell<bool>);
    impl Gate for StopsWhileAsking<'_> {
        fn check(&self, _: Effect) -> Verdict {
            self.0.set(true);
            Verdict::Allow
        }
    }
    stopped.set(false);
    let gate = Stoppable {
        inner: &StopsWhileAsking(&stopped),
        stopped: &is_stopped,
    };
    assert_eq!(gate.check(bash("ls")), Verdict::Deny(STOPPED.into()));
}
