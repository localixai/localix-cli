use std::sync::Mutex;
use std::time::{Duration, Instant};

use contracts::tool::{Grammar, Input, Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use super::*;

/// A tool that says what it was called with, and keeps every call; `fail` refuses its calls.
#[derive(Default)]
struct Echo {
    calls: Mutex<Vec<Value>>,
    fail: Option<ToolError>,
}

impl Tool for Echo {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "echo".into(),
            description: "Echo".into(),
            input: Input::Json(json!({"type": "object"})),
        }
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        crate::lock(&self.calls).push(arguments.clone());
        match &self.fail {
            Some(error) => Err(error.clone()),
            None => Ok(ToolOutput::from(format!("echo {arguments}"))),
        }
    }
}

fn never() -> bool {
    false
}

fn run(tools: &[&dyn Tool], source: &str) -> Result<ToolOutput, ToolError> {
    let exec = ExecTool {
        tools: tools.to_vec(),
        stop: &never,
        offered: Offered::Beside,
    };
    exec.call(&Value::String(source.into()))
}

fn report(result: Result<ToolOutput, ToolError>) -> String {
    match result {
        Ok(output) => output.report,
        Err(error) => panic!("the script failed: {error}"),
    }
}

#[test]
fn what_a_script_prints_and_returns_is_what_the_model_reads() {
    let said = report(run(&[], "text('first'); text(2); return {done: true};"));
    assert_eq!(said, "first\n2\n{\"done\":true}");
    // A string comes back as it is, not quoted.
    assert_eq!(report(run(&[], "return 'plain'")), "plain");
    assert_eq!(report(run(&[], "const x = 1;")), "");
}

#[test]
fn a_script_calls_the_runs_tools_in_order_and_reads_each_result() {
    let echo = Echo::default();
    let said = report(run(
        &[&echo],
        "const a = await tools.echo({n: 1});\n\
         const b = await tools.echo({n: 2, after: a});\n\
         return b;",
    ));
    let calls = echo.calls.lock().unwrap().clone();
    assert_eq!(
        calls,
        vec![json!({"n": 1}), json!({"n": 2, "after": "echo {\"n\":1}"})]
    );
    assert_eq!(said, "echo {\"after\":\"echo {\\\"n\\\":1}\",\"n\":2}");
}

#[test]
fn a_failed_call_is_an_error_the_script_can_catch() {
    let echo = Echo {
        fail: Some(ToolError::Failed("no such file".into())),
        ..Echo::default()
    };
    let said = report(run(
        &[&echo],
        "try { await tools.echo({}); } catch (e) { return 'caught: ' + e.message; }",
    ));
    assert_eq!(said, "caught: no such file");
}

#[test]
fn a_script_that_throws_is_a_failed_call_with_its_message() {
    let Err(ToolError::Failed(message)) = run(&[], "throw new Error('broken here')") else {
        panic!("a throw is a failed call");
    };
    assert!(message.contains("broken here"), "{message}");
    let Err(ToolError::Failed(message)) = run(&[], "return (") else {
        panic!("a syntax error is a failed call");
    };
    assert!(message.contains("SyntaxError"), "{message}");
}

#[test]
fn a_fatal_call_ends_the_script_and_the_run() {
    let echo = Echo {
        fail: Some(ToolError::Fatal("the workspace is gone".into())),
        ..Echo::default()
    };
    let result = run(
        &[&echo],
        "try { await tools.echo({}); } catch (e) { return 'kept going'; }",
    );
    assert!(matches!(result, Err(ToolError::Fatal(_))), "{result:?}");
}

#[test]
fn a_script_reaches_only_the_tools_it_is_given() {
    let said = report(run(
        &[],
        "return [typeof tools.exec, typeof tools.bash, typeof require, typeof process, \
         typeof fetch, typeof console].join(',')",
    ));
    assert_eq!(
        said,
        "undefined,undefined,undefined,undefined,undefined,undefined"
    );
}

#[test]
fn a_stopped_run_stops_a_script_that_never_ends() {
    // The person stops the task a moment after the script started.
    let started = Instant::now();
    let stop = || started.elapsed() > Duration::from_millis(100);
    let exec = ExecTool {
        tools: vec![],
        stop: &stop,
        offered: Offered::Beside,
    };
    let result = exec.call(&Value::String("while (true) {}".into()));
    let Err(ToolError::Failed(message)) = result else {
        panic!("a stopped script is a failed call: {result:?}");
    };
    assert!(message.contains("stopped"), "{message}");
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[test]
fn a_script_past_its_time_is_ended() {
    let result = run(&[], "// @exec: {\"timeout_ms\": 200}\nwhile (true) {}");
    let Err(ToolError::Failed(message)) = result else {
        panic!("a script past its time is a failed call: {result:?}");
    };
    assert!(message.contains("200 ms"), "{message}");
}

#[test]
fn a_script_cannot_take_all_the_memory() {
    let result = run(
        &[],
        "const parts = []; while (true) { parts.push('x'.repeat(1 << 20)); }",
    );
    let Err(ToolError::Failed(message)) = result else {
        panic!("running out of memory is a failed call: {result:?}");
    };
    assert!(message.contains("memory"), "{message}");
}

#[test]
fn a_long_result_keeps_its_start_and_its_end() {
    let said = report(run(&[], "return 'a'.repeat(30000) + 'END'"));
    assert!(said.len() < 12_000, "{}", said.len());
    assert!(said.starts_with("aaa") && said.ends_with("END"));
}

#[test]
fn the_script_is_offered_as_text_with_its_tools_named() {
    let echo = Echo::default();
    let exec = ExecTool {
        tools: vec![&echo],
        stop: &never,
        offered: Offered::Beside,
    };
    let spec = exec.spec();
    assert_eq!(spec.name, "exec");
    assert!(matches!(spec.input, Input::Text(_)));
    assert!(
        spec.description
            .contains("  echo(args: {}): Promise<string>;"),
        "{}",
        spec.description
    );
}

/// A tool with arguments, as `read` has them.
struct Typed;

impl Tool for Typed {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "look".into(),
            description: "Look at a file.".into(),
            input: Input::Json(json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "The file."},
                    "limit": {"type": "integer"},
                    "mode": {"enum": ["fast", "full"]},
                    "tags": {"type": "array", "items": {"type": "string"}},
                },
                "required": ["path"],
            })),
        }
    }

    fn call(&self, _: &Value) -> Result<ToolOutput, ToolError> {
        Ok(ToolOutput::from("looked".to_owned()))
    }
}

#[test]
fn each_tool_is_declared_with_its_arguments() {
    let exec = ExecTool {
        tools: vec![&Typed],
        stop: &never,
        offered: Offered::Beside,
    };
    let description = exec.spec().description;
    for line in [
        "declare const tools: {",
        "  look(args: {",
        "    /** The file. */",
        "    path: string;",
        "    limit?: number;",
        "    mode?: \"fast\" | \"full\";",
        "    tags?: string[];",
        "  }): Promise<string>;",
    ] {
        assert!(description.contains(line), "{line:?} in {description}");
    }
    // Beside the tools, the model already reads each one's description.
    assert!(!description.contains("Look at a file."), "{description}");
}

#[test]
fn offered_alone_it_carries_each_tools_description() {
    let exec = ExecTool {
        tools: vec![&Typed],
        stop: &never,
        offered: Offered::Alone,
    };
    let description = exec.spec().description;
    assert!(
        description.contains("### `tools.look`\nLook at a file."),
        "{description}"
    );
    assert!(description.contains("look(args: {"), "{description}");
    assert!(!description.contains("Not for one call"), "{description}");
}

#[test]
fn a_syntax_error_says_where_it_is() {
    let Err(ToolError::Failed(message)) = run(&[], "const a = 1;\nconst b = f({x: 1);\nreturn b;")
    else {
        panic!("a syntax error is a failed call");
    };
    assert!(message.contains("line 2"), "{message}");
    assert!(message.contains("const b = f({x: 1);"), "{message}");
}

#[test]
fn the_script_is_offered_as_free_text_with_a_grammar_for_its_pragma() {
    let exec = ExecTool {
        tools: vec![],
        stop: &never,
        offered: Offered::Beside,
    };
    let Input::Text(Grammar::Lark(grammar)) = exec.spec().input else {
        panic!("exec takes free text with a grammar");
    };
    assert!(
        grammar.contains("start: pragma_source | plain_source"),
        "{grammar}"
    );
    assert!(grammar.contains("@exec:"), "{grammar}");
}

#[test]
fn a_call_carries_the_script_as_its_text() {
    let exec = ExecTool {
        tools: vec![],
        stop: &never,
        offered: Offered::Beside,
    };
    let text = |source: &str| exec.call(&Value::String(source.into()));
    assert_eq!(report(text("return 1 + 1")), "2");
    // The pragma line is part of the text, as the model writes it.
    assert_eq!(
        report(text("// @exec: {\"timeout_ms\": 5000}\nreturn 'ok'")),
        "ok"
    );
    // Not a script: a failed call that says what the tool takes.
    let error = exec.call(&json!({"source": "return 1"})).unwrap_err();
    assert!(error.to_string().contains("script"), "{error}");
}

#[test]
fn a_script_that_does_not_parse_is_told_what_to_check() {
    let Err(ToolError::Failed(message)) = run(&[], "const r = f({a: \"x\");") else {
        panic!("a syntax error is a failed call");
    };
    assert!(message.contains("did not run"), "{message}");
    assert!(message.contains("template literal"), "{message}");
}

#[test]
fn a_template_literal_that_ran_its_text_as_code_says_how_to_write_it() {
    // The model's patch carries code with its own `${...}`: inside a template literal it runs.
    let Err(ToolError::Failed(message)) = run(&[], "const p = `+ return ${t.date};`;\nreturn p;")
    else {
        panic!("an unknown name is a failed call");
    };
    assert!(message.contains("ReferenceError"), "{message}");
    assert!(message.contains("\\${"), "{message}");
}

#[test]
fn the_line_an_error_shows_is_cut_to_a_readable_length() {
    let long = format!("const a = f({{x: \"{}\");", "y".repeat(5000));
    let Err(ToolError::Failed(message)) = run(&[], &long) else {
        panic!("a syntax error is a failed call");
    };
    assert!(message.len() < 1000, "{}", message.len());
}

/// A tool that may run beside itself: a call waits, up to a while, for another to be running too.
#[derive(Default)]
struct Meeting {
    running: Mutex<usize>,
    met: std::sync::Condvar,
    most: Mutex<usize>,
    alone: bool,
}

impl Tool for Meeting {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "meet".into(),
            description: "Meet".into(),
            input: Input::Json(json!({"type": "object"})),
        }
    }

    fn parallel(&self) -> bool {
        !self.alone
    }

    fn call(&self, _: &Value) -> Result<ToolOutput, ToolError> {
        let mut running = crate::lock(&self.running);
        *running += 1;
        let mut most = crate::lock(&self.most);
        *most = (*most).max(*running);
        drop(most);
        self.met.notify_all();
        let (mut running, _) = self
            .met
            .wait_timeout_while(running, Duration::from_millis(500), |_| {
                *crate::lock(&self.most) < 2
            })
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Met when two ran at once at some point, whichever ended first.
        let said = if *crate::lock(&self.most) >= 2 {
            "met"
        } else {
            "alone"
        };
        *running -= 1;
        Ok(ToolOutput::from(said.to_owned()))
    }
}

#[test]
fn independent_calls_in_one_script_run_at_once() {
    let meeting = Meeting::default();
    let said = report(run(
        &[&meeting],
        "const all = await Promise.all([tools.meet({}), tools.meet({})]); return all.join(',');",
    ));
    assert_eq!(said, "met,met");
}

#[test]
fn a_tool_that_may_not_run_beside_another_runs_alone() {
    let meeting = Meeting {
        alone: true,
        ..Meeting::default()
    };
    let said = report(run(
        &[&meeting],
        "const all = await Promise.all([tools.meet({}), tools.meet({})]); return all.join(',');",
    ));
    assert_eq!(said, "alone,alone");
    assert_eq!(*meeting.most.lock().unwrap(), 1);
}

#[test]
fn settled_calls_report_each_outcome() {
    let echo = Echo {
        fail: Some(ToolError::Failed("no such file".into())),
        ..Echo::default()
    };
    let said = report(run(
        &[&echo],
        "const all = await Promise.allSettled([tools.echo({}), Promise.resolve(1)]);\n\
         return all.map((r) => r.status + ':' + (r.reason ? r.reason.message : r.value)).join(',');",
    ));
    assert_eq!(said, "rejected:no such file,fulfilled:1");
}

#[test]
fn the_description_says_how_calls_run_together_and_what_is_lost() {
    let description = report_spec(Offered::Alone);
    for told in [
        "Promise.all",
        "Promise.allSettled",
        "Promise.race",
        "one at a time",
        "Await every call",
        "Nothing carries over",
    ] {
        assert!(description.contains(told), "{told:?} in {description}");
    }
}

fn report_spec(offered: Offered) -> String {
    let echo = Echo::default();
    let exec = ExecTool {
        tools: vec![&echo],
        stop: &never,
        offered,
    };
    exec.spec().description
}

#[test]
fn a_script_holding_a_nul_character_says_where() {
    let Err(ToolError::Failed(message)) =
        run(&[], "const a = 1;\nconst b = a ? \u{0} : 2;\nreturn b;")
    else {
        panic!("a NUL is a failed call");
    };
    assert!(message.contains("line 2"), "{message}");
    assert!(message.contains("NUL"), "{message}");
}
