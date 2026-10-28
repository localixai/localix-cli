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
    let why = denied(gate.check(effect("apply_patch")));
    assert!(
        why.contains("`apply_patch`") && why.contains("plan"),
        "{why}"
    );
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
    assert_eq!(gate.check(effect("jobs")), Verdict::Allow);
    assert_eq!(count(), 3, "only bash was always");
}

#[test]
fn bypass_allows_without_asking() {
    let mode = ModeCell::new(built("bypass"));
    let typed = Typed::new(&[]);
    let waited = |_: Duration| panic!("nobody waited");
    let gate = ModeGate::new(&mode, Some(&typed), &waited);
    for tool in ["apply_patch", "bash", "jobs"] {
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
        text.starts_with("plan:") && text.contains("`apply_patch`"),
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
