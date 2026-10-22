use std::cell::RefCell;

use super::*;
use crate::testing::Typed;

fn effect(tool: &str) -> Effect<'_> {
    Effect {
        tool,
        summary: "the summary",
    }
}

fn denied(verdict: Verdict) -> String {
    match verdict {
        Verdict::Deny(why) => why,
        Verdict::Allow => panic!("allowed"),
    }
}

#[test]
fn yes_is_bypass_and_otherwise_the_flag_wins_over_the_default() {
    assert_eq!(Mode::choose(None, true), Mode::Bypass);
    assert_eq!(Mode::choose(Some(Mode::Plan), false), Mode::Plan);
    assert_eq!(Mode::choose(None, false), Mode::Default);
    assert_eq!(Mode::named(" Plan "), Some(Mode::Plan));
    assert_eq!(Mode::named("nope"), None);
    for mode in Mode::ALL {
        assert_eq!(Mode::named(mode.name()), Some(mode));
        assert!(!mode.hint().is_empty(), "{mode:?} has no hint");
    }
}

#[test]
fn default_asks_at_the_terminal_and_refuses_without_one() {
    let mode = Cell::new(Mode::Default);
    let typed = Typed::new(&["y\n"]);
    let waited = |_: Duration| {};
    let gate = ModeGate::new(&mode, Some(&typed), &waited);
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(typed.asked.borrow().len(), 1);
    let gate = ModeGate::new::<&Typed>(&mode, None, &waited);
    assert_eq!(denied(gate.check(effect("bash"))), NO_TERMINAL);
    assert!(NO_TERMINAL.contains("--yes (-y) or --mode bypass"));
}

#[test]
fn plan_refuses_edits_without_asking() {
    let mode = Cell::new(Mode::Plan);
    let typed = Typed::new(&["y\n"]);
    let waited = |_: Duration| {};
    let gate = ModeGate::new(&mode, Some(&typed), &waited);
    assert_eq!(denied(gate.check(effect("edit"))), PLAN_REFUSAL);
    assert!(typed.asked.borrow().is_empty(), "the person was not asked");
    let gate = ModeGate::new::<&Typed>(&mode, None, &waited);
    assert_eq!(denied(gate.check(effect("edit"))), PLAN_REFUSAL);
}

#[test]
fn plan_with_no_terminal_never_points_to_bypass() {
    let mode = Cell::new(Mode::Plan);
    let waited = |_: Duration| {};
    let gate = ModeGate::new::<&Typed>(&mode, None, &waited);
    for tool in ["bash", "browser"] {
        let why = denied(gate.check(effect(tool)));
        assert_eq!(why, PLAN_NO_TERMINAL, "{tool}");
        assert!(!why.contains("bypass") && !why.contains("--yes"), "{why}");
    }
    assert!(!describe(Mode::Plan, false, false).contains("bypass"));
}

#[test]
fn plan_keeps_its_own_always_answers() {
    let mode = Cell::new(Mode::Default);
    let typed = Typed::new(&["a\n", "y\n", "a\n"]);
    let waited = |_: Duration| {};
    let gate = ModeGate::new(&mode, Some(&typed), &waited);
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    mode.set(Mode::Plan);
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(
        typed.asked.borrow().len(),
        2,
        "plan asked despite the default's always"
    );
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(typed.asked.borrow().len(), 3, "plan's own always holds");
    mode.set(Mode::Default);
    assert_eq!(
        gate.check(effect("browser")),
        Verdict::Deny("no answer came from the terminal".into())
    );
    assert_eq!(
        typed.asked.borrow().len(),
        4,
        "the default's always is only for bash"
    );
}

#[test]
fn bypass_and_autonomous_allow_without_asking() {
    for chosen in [Mode::Bypass, Mode::Autonomous] {
        let mode = Cell::new(chosen);
        let typed = Typed::new(&[]);
        let waited = |_: Duration| panic!("nobody waited");
        let gate = ModeGate::new(&mode, Some(&typed), &waited);
        assert_eq!(gate.check(effect("edit")), Verdict::Allow);
        assert_eq!(gate.check(effect("bash")), Verdict::Allow);
        assert!(typed.asked.borrow().is_empty());
    }
}

#[test]
fn the_time_the_person_takes_is_told() {
    let mode = Cell::new(Mode::Default);
    let typed = Typed::new(&["n\n"]);
    let told = RefCell::new(vec![]);
    let waited = |took: Duration| told.borrow_mut().push(took);
    let gate = ModeGate::new(&mode, Some(&typed), &waited);
    assert!(matches!(gate.check(effect("bash")), Verdict::Deny(_)));
    assert_eq!(told.borrow().len(), 1);
}

#[test]
fn the_context_mentions_ask_only_where_someone_answers() {
    for mode in Mode::ALL {
        let text = describe(mode, true, true);
        assert!(text.starts_with(mode.name()), "{text}");
        let mentions = text.contains("`ask`");
        assert_eq!(mentions, mode != Mode::Autonomous, "{text}");
        assert!(
            !describe(mode, true, false).contains("`ask`"),
            "{mode:?} in exec"
        );
        assert!(
            !describe(mode, false, true).contains("`ask`"),
            "{mode:?} with no terminal"
        );
    }
    assert!(describe(Mode::Default, false, false).contains("--mode bypass"));
    assert!(describe(Mode::Plan, true, false).contains("`edit` is refused"));
    assert!(describe(Mode::Plan, false, false).contains("plan from reading files"));
    assert!(describe(Mode::Autonomous, true, true).contains("nobody answers questions"));
}

#[test]
fn a_change_is_a_mode_change_note() {
    let note = notice(Mode::Plan, true, true);
    assert!(
        note.starts_with("<mode_change>\nThe mode is now plan: "),
        "{note}"
    );
    assert!(note.ends_with("no longer holds.\n</mode_change>"), "{note}");
}

#[test]
fn the_list_marks_the_current_mode() {
    let list = list(Mode::Plan);
    assert_eq!(list.lines().count(), 4);
    assert!(
        list.contains("* plan        Investigate and plan"),
        "{list}"
    );
    assert!(list.contains("  default     "), "{list}");
    assert!(switched(Mode::Bypass).starts_with("Mode: bypass. Everything runs without asking"));
}

/// `bash`'s terminal sessions through the mode's gate. Starting one and typing into it are
/// effects like any command, so each mode answers them as it answers a command: plan asks for
/// every start and every input (a session is no way around its approvals), bypass and autonomous
/// allow, and with no terminal they are refused. Reading a session and closing it ask nothing.
#[cfg(unix)]
mod sessions {
    use contracts::tool::{Tool, ToolError};
    use serde_json::{Value, json};
    use tools::shell::{BashTool, Sessions};
    use tools::workspace::Workspace;

    use super::*;
    use crate::testing::Scratch;

    /// A shell started, a file-making line typed into it, the session read and closed. The call
    /// that types waits for `42`, which only the shell prints once `touch` is done: the echo of
    /// what is typed reads `40+2`, so the wait cannot end on it.
    fn calls() -> [Value; 4] {
        [
            json!({"command": "sh", "tty": true, "yield_ms": 300}),
            json!({"session": 1, "input": "touch made && echo $((40+2))\n", "wait_for": "42"}),
            json!({"session": 1, "yield_ms": 100}),
            json!({"session": 1, "close": true}),
        ]
    }

    /// Each call's report, or why it failed; and whether the typed line made its file.
    fn run(mode: Mode, terminal: Option<&Typed>) -> (Vec<Result<String, String>>, bool) {
        let scratch = Scratch::new();
        let workspace = Workspace::new(scratch.path()).unwrap();
        let mode = Cell::new(mode);
        let waited = |_: Duration| {};
        let gate = ModeGate::new(&mode, terminal, &waited);
        let sessions = Sessions::default();
        let bash = BashTool {
            workspace: &workspace,
            gate: &gate,
            hidden: &[],
            stop: &|| false,
            sessions: &sessions,
        };
        let results = calls()
            .iter()
            .map(|arguments| match bash.call(arguments) {
                Ok(output) => Ok(output.report),
                Err(ToolError::Failed(why) | ToolError::Fatal(why)) => Err(why),
            })
            .collect();
        (results, scratch.path().join("made").exists())
    }

    #[test]
    fn plan_asks_for_every_start_and_input() {
        let typed = Typed::new(&["y\n", "y\n"]);
        let (results, made) = run(Mode::Plan, Some(&typed));
        assert!(results.iter().all(Result::is_ok), "{results:?}");
        let asked = typed.asked.borrow();
        assert_eq!(asked.len(), 2, "reading and closing ask nothing: {asked:?}");
        assert!(asked[0].contains("sh  (in a terminal)"), "{}", asked[0]);
        assert!(
            asked[1].contains("type into session 1 (`sh`): touch made"),
            "{}",
            asked[1]
        );
        assert!(made, "what the person approved ran");
    }

    #[test]
    fn plan_types_nothing_the_person_declined() {
        let typed = Typed::new(&["y\n", "n\n"]);
        let (results, made) = run(Mode::Plan, Some(&typed));
        assert!(results[0].is_ok(), "{results:?}");
        assert!(
            results[1]
                .as_ref()
                .is_err_and(|why| why.contains("declined")),
            "{results:?}"
        );
        assert!(!made, "a declined input changed a file");
    }

    #[test]
    fn with_no_terminal_a_session_is_refused_in_default_and_plan() {
        for (mode, why) in [(Mode::Default, NO_TERMINAL), (Mode::Plan, PLAN_NO_TERMINAL)] {
            let (results, made) = run(mode, None);
            assert_eq!(results[0], Err(format!("not allowed: {why}")), "{mode:?}");
            assert!(!made);
        }
    }

    #[test]
    fn bypass_and_autonomous_run_a_session_without_asking() {
        for mode in [Mode::Bypass, Mode::Autonomous] {
            let typed = Typed::new(&[]);
            let (results, made) = run(mode, Some(&typed));
            assert!(results.iter().all(Result::is_ok), "{mode:?}: {results:?}");
            assert!(typed.asked.borrow().is_empty(), "{mode:?}");
            assert!(made, "{mode:?}");
        }
    }

    #[test]
    fn default_asks_like_plain_bash_and_always_covers_the_session() {
        let typed = Typed::new(&["a\n"]);
        let (results, made) = run(Mode::Default, Some(&typed));
        assert!(results.iter().all(Result::is_ok), "{results:?}");
        assert_eq!(
            typed.asked.borrow().len(),
            1,
            "`a` for bash holds for typing too"
        );
        assert!(made);
    }
}
