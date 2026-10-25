use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::gate::Asker;
use crate::modes::built;
use crate::testing::Typed;
use protocol::Approval;

fn effect(tool: &str) -> Effect<'_> {
    Effect {
        tool,
        subject: "",
        summary: "the summary",
    }
}

fn command(line: &str) -> Effect<'_> {
    Effect {
        tool: "bash",
        subject: line,
        summary: line,
    }
}

fn denied(verdict: Verdict) -> String {
    match verdict {
        Verdict::Deny(why) => why,
        Verdict::Allow => panic!("allowed"),
    }
}

#[test]
fn default_asks_at_the_terminal_and_refuses_without_one() {
    let mode = ModeCell::new(built("default"));
    let typed = Typed::new(&["y\n"]);
    let waited = |_: Duration| {};
    let gate = ModeGate::new(&mode, Some(&typed), &waited);
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(typed.asked.lock().unwrap().len(), 1);
    let gate = ModeGate::new::<&Typed>(&mode, None, &waited);
    assert_eq!(denied(gate.check(effect("bash"))), NO_ONE);
}

#[test]
fn plan_refuses_edits_by_its_rule_without_asking() {
    let mode = ModeCell::new(built("plan"));
    let typed = Typed::new(&["y\n"]);
    let waited = |_: Duration| {};
    let gate = ModeGate::new(&mode, Some(&typed), &waited);
    let why = denied(gate.check(effect("edit")));
    assert!(why.contains("`edit`") && why.contains("plan"), "{why}");
    assert!(
        typed.asked.lock().unwrap().is_empty(),
        "the person was not asked"
    );
}

#[test]
fn the_command_lines_rules_join_the_modes() {
    let mode = ModeCell::new(built("plan"));
    let waited = |_: Duration| {};
    let rules = Rules::new(&["bash(cargo test *)".into()], &["bash(git push *)".into()]).unwrap();
    let gate = ModeGate::new::<&Typed>(&mode, None, &waited).ruled(rules);
    assert_eq!(gate.check(command("cargo test -q")), Verdict::Allow);
    assert_eq!(denied(gate.check(command("cargo build"))), NO_ONE);
    mode.set(built("bypass"));
    assert!(denied(gate.check(command("git push"))).contains("bash(git push *)"));
    assert_eq!(gate.check(command("git status")), Verdict::Allow);
}

/// An asker whose person answers "always" each time, counted.
struct Always<'a>(&'a AtomicUsize);

impl Asker for Always<'_> {
    fn approve(&self, _: &Effect) -> Option<Approval> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(Approval::Always)
    }
}

#[test]
fn always_is_kept_by_mode() {
    let mode = ModeCell::new(built("default"));
    let asked = AtomicUsize::new(0);
    let count = || asked.load(Ordering::SeqCst);
    let gate = ModeGate::asking(&mode, Some(Box::new(Always(&asked))), &|_| {});
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(count(), 1, "always holds across calls");
    mode.set(built("plan"));
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(count(), 2, "plan asks once for itself");
    mode.set(built("default"));
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(gate.check(effect("browser")), Verdict::Allow);
    assert_eq!(count(), 3, "only bash was always");
}

#[test]
fn bypass_allows_without_asking() {
    let mode = ModeCell::new(built("bypass"));
    let typed = Typed::new(&[]);
    let waited = |_: Duration| panic!("nobody waited");
    let gate = ModeGate::new(&mode, Some(&typed), &waited);
    for tool in ["edit", "bash", "browser", "terminal", "jobs"] {
        assert_eq!(gate.check(effect(tool)), Verdict::Allow, "{tool}");
    }
    assert!(typed.asked.lock().unwrap().is_empty());
}

#[test]
fn the_time_the_person_takes_is_told() {
    let mode = ModeCell::new(built("default"));
    let typed = Typed::new(&["n\n"]);
    let told = Mutex::new(vec![]);
    let waited = |took: Duration| told.lock().unwrap().push(took);
    let gate = ModeGate::new(&mode, Some(&typed), &waited);
    assert!(matches!(gate.check(effect("bash")), Verdict::Deny(_)));
    assert_eq!(told.lock().unwrap().len(), 1);
}

#[test]
fn the_context_states_the_rules_and_what_becomes_of_the_rest() {
    let none = Rules::default();
    let plan = built("plan");
    let text = describe(&plan, &none, false, false);
    assert!(
        text.starts_with("plan:") && text.contains("`edit`"),
        "{text}"
    );
    assert!(text.contains("is refused"), "{text}");
    assert!(describe(&plan, &none, true, false).contains("is asked"));
    let more = Rules::new(&["bash(cargo test *)".into()], &[]).unwrap();
    assert!(describe(&plan, &more, false, false).contains("bash(cargo test *)"));
    assert!(describe(&plan, &none, true, true).contains("`ask`"));
    assert!(
        !describe(&plan, &none, true, false).contains("`ask`"),
        "in exec"
    );
    assert!(
        !describe(&plan, &none, false, true).contains("`ask`"),
        "with no terminal"
    );
}

/// `bash`'s terminal sessions through the mode's gate. Starting one and typing into it are
/// effects like any command, so each mode answers them as it answers a command: plan asks for
/// every start and every input (a session is no way around its approvals), bypass and autonomous
/// allow, and with no terminal they are refused. Reading a session and closing it ask nothing.
#[cfg(unix)]
mod sessions {
    use contracts::tool::ToolError;
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
        let mode = ModeCell::new(mode);
        let waited = |_: Duration| {};
        let gate = ModeGate::new(&mode, terminal, &waited);
        let sessions = Sessions::default();
        let bash = BashTool {
            workspace: &workspace,
            gate: &gate,
            hidden: &Vec::new,
            stop: &|| false,
            sessions: &sessions,
        };
        let results = calls()
            .iter()
            .map(
                |arguments| match bash.terminal(arguments).map(|(output, _)| output) {
                    Ok(output) => Ok(output.report),
                    Err(ToolError::Failed(why) | ToolError::Fatal(why)) => Err(why),
                },
            )
            .collect();
        (results, scratch.path().join("made").exists())
    }

    #[test]
    fn plan_asks_for_every_start_and_input() {
        let typed = Typed::new(&["y\n", "y\n"]);
        let (results, made) = run(built("plan"), Some(&typed));
        assert!(results.iter().all(Result::is_ok), "{results:?}");
        let asked = typed.asked.lock().unwrap();
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
        let (results, made) = run(built("plan"), Some(&typed));
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
        for mode in ["default", "plan"] {
            let (results, made) = run(built(mode), None);
            assert_eq!(results[0], Err(format!("not allowed: {NO_ONE}")), "{mode}");
            assert!(!made);
        }
    }

    #[test]
    fn bypass_runs_a_session_without_asking() {
        let typed = Typed::new(&[]);
        let (results, made) = run(built("bypass"), Some(&typed));
        assert!(results.iter().all(Result::is_ok), "{results:?}");
        assert!(typed.asked.lock().unwrap().is_empty());
        assert!(made);
    }

    #[test]
    fn default_asks_like_plain_bash_and_always_covers_the_session() {
        let typed = Typed::new(&["a\n"]);
        let (results, made) = run(built("default"), Some(&typed));
        assert!(results.iter().all(Result::is_ok), "{results:?}");
        assert_eq!(
            typed.asked.lock().unwrap().len(),
            1,
            "`a` for bash holds for typing too"
        );
        assert!(made);
    }
}
