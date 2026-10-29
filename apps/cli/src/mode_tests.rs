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
        Verdict::Allow | Verdict::ReadOnly => panic!("allowed"),
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
fn plan_runs_commands_read_only_without_asking_and_its_deny_rules_still_refuse() {
    let mode = ModeCell::new(built("plan"));
    let typed = Typed::new(&["y\n"]);
    let waited = |_: Duration| {};
    let gate = ModeGate::new(&mode, Some(&typed), &waited);
    assert_eq!(gate.check(command("rm -rf src")), Verdict::ReadOnly);
    assert_eq!(gate.check(effect("jobs")), Verdict::ReadOnly);
    assert!(typed.asked.lock().unwrap().is_empty(), "nobody was asked");
    let more = Rules::new(&["bash".into()], &["bash(curl *)".into()]).unwrap();
    let gate = ModeGate::new::<&Typed>(&mode, None, &waited).ruled(more);
    assert_eq!(
        gate.check(command("cargo test")),
        Verdict::ReadOnly,
        "an allow does not open it"
    );
    assert!(denied(gate.check(command("curl x"))).contains("plan"));
}

#[test]
fn the_command_lines_rules_join_the_modes() {
    let mode = ModeCell::new(built("default"));
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
    mode.set(Mode::new("review", "", Rules::default(), Reach::Anything));
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(count(), 2, "review asks once for itself");
    mode.set(built("default"));
    assert_eq!(gate.check(effect("bash")), Verdict::Allow);
    assert_eq!(gate.check(effect("jobs")), Verdict::Allow);
    assert_eq!(count(), 3, "only bash was always");
}

#[test]
fn bypass_allows_without_asking() {
    let mode = ModeCell::new(built("bypass"));
    let typed = Typed::new(&[]);
    let waited = |_: Duration| panic!("nobody waited");
    let gate = ModeGate::new(&mode, Some(&typed), &waited);
    for tool in ["bash", "jobs"] {
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
    let default = built("default");
    let more = Rules::new(&["bash(cargo test *)".into()], &[]).unwrap();
    let text = describe(&default, &more, false, false);
    assert!(
        text.starts_with("default:") && text.contains("bash(cargo test *)"),
        "{text}"
    );
    assert!(text.contains("is refused"), "{text}");
    assert!(describe(&default, &none, true, false).contains("is asked"));
    assert!(describe(&default, &none, true, true).contains("`ask`"));
    assert!(
        !describe(&default, &none, true, false).contains("`ask`"),
        "in exec"
    );
    assert!(
        !describe(&default, &none, false, true).contains("`ask`"),
        "with no terminal"
    );
}

#[test]
fn a_mode_that_allows_every_tool_refuses_nothing_in_its_text() {
    let none = Rules::default();
    for terminal in [true, false] {
        let text = describe(&built("bypass"), &none, terminal, false);
        assert!(
            text.starts_with("bypass:") && text.contains("whatever they do"),
            "{text}"
        );
        assert!(
            !text.contains("is asked")
                && !text.contains("is refused")
                && !text.contains("every part"),
            "{text}"
        );
    }
    assert!(describe(&built("bypass"), &none, true, true).contains("`ask`"));
}

#[test]
fn a_read_only_mode_tells_the_model_its_commands_change_nothing() {
    let none = Rules::default();
    for terminal in [true, false] {
        let text = describe(&built("plan"), &none, terminal, false);
        assert!(
            text.starts_with("plan:") && text.contains("sandbox"),
            "{text}"
        );
        assert!(
            !text.contains("is asked") && !text.contains("is refused"),
            "{text}"
        );
    }
}
