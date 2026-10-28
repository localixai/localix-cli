//! `exec`, the script that chains the tools (ARCHITECTURE.md X1-X6), as a task meets it: offered
//! beside the other tools (an agent's too), its calls asking the gate as a direct call does, and
//! each shown in the trace and the log under the `exec` call.

use serde_json::{Value, json};

use crate::common::home::{Home, text};
use crate::common::model::{Model, Reply, said};
use crate::suite::exec::exec;

fn script(source: &str) -> Reply {
    Reply::call("exec", json!({ "source": source }))
}

fn names(request: &Value) -> Vec<String> {
    (request["tools"].as_array().unwrap().iter())
        .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_script_reads_and_the_model_gets_back_only_what_it_returned() {
    let home = Home::new();
    std::fs::write(home.path().join("big.txt"), "filler\n".repeat(300)).unwrap();
    let model = Model::start(vec![
        script("const t = await tools.read({path: 'big.txt'}); return t.split('\\n').length;"),
        Reply::text("Counted.\nRESULT: achieved"),
    ]);
    let out = exec(&home, &model, &["exec", "-y", "count"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let tools = names(&model.requests()[0]);
    assert!(tools.contains(&"exec".into()), "{tools:?}");
    let back = said(&model, 1);
    assert!(
        !back.contains("filler"),
        "the file did not travel back: {back}"
    );
    assert!(back.contains("301"), "the count did: {back}");
}

#[test]
fn a_command_from_a_script_asks_the_gate_like_a_direct_one() {
    // No `-y`, no one to ask: a direct `bash` is refused, and so is the script's.
    let home = Home::new();
    let source = "try { await tools.bash({command: 'touch ran'}); return 'ran'; } \
                  catch (e) { return 'caught: ' + e.message; }";
    let model = Model::start(vec![script(source), Reply::text("Done.\nRESULT: achieved")]);
    let out = exec(&home, &model, &["exec", "touch"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(!home.path().join("ran").exists());
    assert!(said(&model, 1).contains("caught:"), "{}", said(&model, 1));
}

#[test]
fn a_script_reaches_no_tool_that_changes_a_file() {
    let home = Home::new();
    let source = "return [typeof tools.read, typeof tools.bash, typeof tools.edit, \
                  typeof tools.apply_patch].join(' ');";
    let model = Model::start(vec![script(source), Reply::text("Done.\nRESULT: achieved")]);
    let out = exec(&home, &model, &["exec", "-y", "x"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let back = said(&model, 1);
    assert!(
        back.contains("function function undefined undefined"),
        "{back}"
    );
}

#[test]
fn each_call_of_a_script_shows_in_the_trace_and_the_log_under_exec() {
    let home = Home::new();
    std::fs::write(home.path().join("a.txt"), "hello\n").unwrap();
    let source = "await tools.read({path: 'a.txt'}); await tools.bash({command: 'echo hi'});";
    let model = Model::start(vec![script(source), Reply::text("Done.\nRESULT: achieved")]);
    let out = exec(&home, &model, &["exec", "-y", "look"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let stderr = text(&out.stderr);
    assert!(stderr.contains("exec › Reading a.txt"), "{stderr}");
    assert!(stderr.contains("exec › Running echo hi"), "{stderr}");
    let log = std::fs::read_to_string(home.path().join("config/llx/logs/llx.log")).unwrap();
    assert!(log.contains(r#"exec › read {"path":"a.txt"}"#), "{log}");
    assert!(log.contains("exec › bash"), "{log}");
}

#[test]
fn an_agent_is_offered_exec_with_the_tools_of_the_main_session() {
    let home = Home::new();
    let model = Model::start(vec![Reply::call(
        "agents",
        json!({"action": "spawn", "task": "say hi", "name": "scout"}),
    )]);
    let out = exec(&home, &model, &["exec", "-y", "start scout"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let requests = model.requests();
    assert!(
        requests.len() >= 3,
        "the agent asked too: {}",
        requests.len()
    );
    let main = names(&requests[0]);
    assert!(main.contains(&"exec".into()), "{main:?}");
    for request in &requests {
        assert_eq!(names(request), main, "one list, one prompt cache");
    }
}

const PATCH: &str = "*** Begin Patch\n*** Update File: a.txt\n@@\n-one\n+two\n\
                     *** Add File: sub/b.txt\n+new\n*** End Patch";

/// The model's patch, as free text: one call that changes every file it names.
fn patch() -> Reply {
    Reply::Calls(vec![("apply_patch".into(), json!({ "source": PATCH }))])
}

#[test]
fn a_patch_changes_two_files_in_one_call() {
    let home = Home::new();
    std::fs::write(home.path().join("a.txt"), "one\n").unwrap();
    let model = Model::start(vec![patch(), Reply::text("Done.\nRESULT: achieved")]);
    let out = exec(&home, &model, &["exec", "-y", "patch"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(
        std::fs::read_to_string(home.path().join("a.txt")).unwrap(),
        "two\n"
    );
    assert_eq!(
        std::fs::read_to_string(home.path().join("sub/b.txt")).unwrap(),
        "new\n"
    );
    let back = said(&model, 1);
    assert!(
        back.contains("Updated a.txt") && back.contains("Created sub/b.txt"),
        "{back}"
    );
    let stderr = text(&out.stderr);
    assert!(stderr.contains("Patched a.txt, sub/b.txt"), "{stderr}");
}

#[test]
fn a_patch_cannot_change_files_in_plan_mode() {
    let home = Home::new();
    std::fs::write(home.path().join("a.txt"), "one\n").unwrap();
    let model = Model::start(vec![patch(), Reply::text("Done.\nRESULT: achieved")]);
    let out = exec(&home, &model, &["exec", "--mode", "plan", "x"], "");
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(
        std::fs::read_to_string(home.path().join("a.txt")).unwrap(),
        "one\n"
    );
    assert!(!home.path().join("sub/b.txt").exists());
    assert!(
        said(&model, 1).contains("refused by the rule `apply_patch`"),
        "{}",
        said(&model, 1)
    );
}
