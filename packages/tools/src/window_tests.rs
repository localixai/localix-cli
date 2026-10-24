use std::sync::Mutex;

use serde_json::json;

use contracts::gate::Open;
use contracts::hands::{Hands, Outcome, Report, Task};
use contracts::view::{Action, Observation, View, ViewResult};

use super::*;
use crate::testing::Recorder;

/// A kind of tab that records what it was asked, and opens tabs 1, 2, ...
struct Fake {
    name: &'static str,
    letter: char,
    open: Mutex<Vec<u32>>,
    done: Mutex<Vec<String>>,
}

impl Fake {
    fn new(name: &'static str, letter: char) -> Self {
        Self {
            name,
            letter,
            open: Mutex::new(vec![]),
            done: Mutex::new(vec![]),
        }
    }

    fn did(&self, what: String) {
        self.done.lock().unwrap().push(what);
    }

    #[allow(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn check(&self, id: u32) -> Result<(), ToolError> {
        let open = self.open.lock().unwrap().contains(&id);
        match open {
            true => Ok(()),
            false => Err(ToolError::Failed(format!("no {} tab {id}", self.name))),
        }
    }
}

impl Kind for Fake {
    fn name(&self) -> &'static str {
        self.name
    }
    fn letter(&self) -> char {
        self.letter
    }
    fn guide(&self) -> &'static str {
        "how to use it"
    }
    fn fields(&self) -> Vec<(&'static str, Value)> {
        vec![("thing", json!({"type": "string"}))]
    }
    #[allow(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn open(&self, args: &Value) -> Result<Opened, ToolError> {
        let mut open = self.open.lock().unwrap();
        let id = u32::try_from(open.len()).unwrap() + 1;
        open.push(id);
        self.did(format!("open {}", args["thing"]));
        Ok(Opened {
            id,
            output: ToolOutput {
                report: format!("opened {}", args["thing"]),
                state: Some(format!("snapshot of {id}")),
                image: None,
            },
        })
    }
    fn interact(&self, id: u32, args: &Value) -> Result<ToolOutput, ToolError> {
        self.check(id)?;
        self.did(format!("interact {id} {}", args["thing"]));
        Ok(ToolOutput {
            report: "done".into(),
            state: Some(format!("snapshot of {id} after")),
            image: None,
        })
    }
    fn snapshot(&self, id: u32) -> Result<ToolOutput, ToolError> {
        self.check(id)?;
        Ok(ToolOutput {
            report: String::new(),
            state: Some(format!("snapshot of {id}")),
            image: None,
        })
    }
    fn screenshot(&self, id: u32, _: &Value) -> Result<ToolOutput, ToolError> {
        self.check(id)?;
        self.did(format!("screenshot {id}"));
        Ok("a picture".to_string().into())
    }
    #[allow(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn close(&self, id: u32) -> Result<String, ToolError> {
        self.check(id)?;
        self.open.lock().unwrap().retain(|open| *open != id);
        Ok(format!("closed {id}"))
    }
    fn tabs(&self) -> Vec<(u32, String, String)> {
        let open = self.open.lock().unwrap();
        open.iter()
            .map(|id| (*id, format!("{} {id}", self.name), String::new()))
            .collect()
    }
    fn view(&self, id: u32) -> Result<Box<dyn View + '_>, ToolError> {
        self.check(id)?;
        Ok(Box::new(Screen(format!("{} {id}", self.name))))
    }
}

/// A tab's view: one line of text.
struct Screen(String);

impl View for Screen {
    fn observe(&self) -> ViewResult<Observation> {
        Ok(Observation {
            title: self.0.clone(),
            ..Observation::default()
        })
    }
    fn act(&self, _: &str, _: &Action) -> ViewResult<()> {
        Ok(())
    }
}

/// Hands that say what view they were given and finish.
#[derive(Default)]
struct Helper {
    saw: Mutex<Vec<String>>,
}

impl Hands for Helper {
    fn carry_out(&self, view: &dyn View, task: Task) -> Report {
        let title = view.observe().unwrap().title;
        self.saw
            .lock()
            .unwrap()
            .push(format!("{title}: {}", task.goal));
        Report {
            steps: vec![format!(
                "TYPE textbox \"terminal\" = \"{}\"",
                task.values[0]
            )],
            outcome: Outcome::Done { probability: 0.9 },
            view: None,
        }
    }
}

/// A window that shows what it is told to, and keeps what it was asked.
#[derive(Default)]
struct Shown {
    asked: Mutex<Vec<String>>,
    /// Tabs beside its own, of other projects.
    others: Vec<WindowTab>,
}

impl Windowed for Shown {
    fn tabs(&self) -> Option<Vec<WindowTab>> {
        Some(
            vec![WindowTab {
                id: "w1".into(),
                kind: "browser".into(),
                title: "Example".into(),
                detail: "https://example.org/".into(),
                shown: true,
                acted_on: true,
                group: "llx".into(),
            }]
            .into_iter()
            .chain(self.others.clone())
            .collect(),
        )
    }
    fn opened(&self, tab: &str) {
        self.asked.lock().unwrap().push(format!("opened {tab}"));
    }
    #[allow(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn act(&self, act: Act, tab: &str) -> Result<(), String> {
        self.asked.lock().unwrap().push(format!("{act:?} {tab}"));
        Ok(())
    }
    #[allow(
        clippy::unwrap_in_result,
        reason = "a test's fake: a poisoned lock fails the test"
    )]
    fn resize(&self, width: u32, height: u32) -> Result<(), String> {
        self.asked
            .lock()
            .unwrap()
            .push(format!("resize {width}×{height}"));
        Ok(())
    }
}

fn call(tool: &WindowTool, arguments: Value) -> Result<ToolOutput, ToolError> {
    tool.call(&arguments)
}

#[test]
fn open_takes_several_tabs_of_any_kind_and_gives_each_ones_snapshot() {
    let (web, term) = (Fake::new("browser", 'w'), Fake::new("terminal", 't'));
    let tool = WindowTool {
        kinds: vec![&web, &term],
        window: None,
        hands: None,
        gate: &Open,
    };
    let done = call(
        &tool,
        json!({"action": "open", "tabs": [
            {"kind": "browser", "thing": "a"},
            {"kind": "terminal", "thing": "b"},
            {"kind": "browser", "thing": "c"}
        ]}),
    )
    .unwrap();
    // The reports keep the order asked; which browser tab got which number is theirs to race for.
    let lines: Vec<&str> = done.report.lines().collect();
    let (first, second) = (lines[0], lines[2]);
    assert_eq!(lines[1], "t1 (terminal): opened \"b\"");
    assert!(first.ends_with("(browser): opened \"a\""), "{first}");
    assert!(second.ends_with("(browser): opened \"c\""), "{second}");
    let name = |line: &str| line.split(' ').next().unwrap_or_default().to_string();
    let mut browsers = [name(first), name(second)];
    browsers.sort_unstable();
    assert_eq!(browsers, ["w1", "w2"], "{}", done.report);
    let state = done.state.unwrap();
    assert!(
        state.contains("### w1\nsnapshot of 1") && state.contains("### w2\nsnapshot of 2"),
        "{state}"
    );
    // One tab may be named at the top level too.
    let one = call(
        &tool,
        json!({"action": "open", "kind": "terminal", "thing": "d"}),
    )
    .unwrap();
    assert_eq!(one.report, "t2 (terminal): opened \"d\"");
}

#[test]
fn interact_goes_to_the_tabs_kind_and_comes_back_with_its_snapshot() {
    let (web, term) = (Fake::new("browser", 'w'), Fake::new("terminal", 't'));
    let tool = WindowTool {
        kinds: vec![&web, &term],
        window: None,
        hands: None,
        gate: &Open,
    };
    call(
        &tool,
        json!({"action": "open", "kind": "terminal", "thing": "sh"}),
    )
    .unwrap();
    let done = call(
        &tool,
        json!({"action": "interact", "tab": "t1", "thing": "ls"}),
    )
    .unwrap();
    assert_eq!(done.report, "t1: done");
    assert_eq!(done.state.as_deref(), Some("snapshot of 1 after"));
    assert_eq!(
        *term.done.lock().unwrap(),
        ["open \"sh\"", "interact 1 \"ls\""]
    );
    assert!(web.done.lock().unwrap().is_empty());
}

#[test]
fn show_puts_a_tab_in_front_and_reads_it_and_close_takes_several() {
    let (web, term) = (Fake::new("browser", 'w'), Fake::new("terminal", 't'));
    let window = Shown::default();
    let tool = WindowTool {
        kinds: vec![&web, &term],
        window: Some(&window),
        hands: None,
        gate: &Open,
    };
    let opened = call(
        &tool,
        json!({"action": "open", "tabs": [{"kind": "terminal"}, {"kind": "terminal"}]}),
    )
    .unwrap();
    // The tabs open at once: which gets which number is theirs to race for; the report says.
    let tabs: Vec<&str> = (opened.report.lines())
        .filter_map(|line| line.split_once(' ').map(|(tab, _)| tab))
        .collect();
    let mut numbered = tabs.clone();
    numbered.sort_unstable();
    assert_eq!(numbered, ["t1", "t2"], "{}", opened.report);
    let second = tabs[1];
    let shown = call(&tool, json!({"action": "show", "tab": second})).unwrap();
    assert_eq!(shown.report, format!("Showing {second}."));
    let number = second.trim_start_matches('t');
    assert_eq!(shown.state, Some(format!("snapshot of {number}")));
    let closed = call(&tool, json!({"action": "close", "tabs": ["t1", "t2"]})).unwrap();
    assert_eq!(closed.report, "t1: closed 1\nt2: closed 2");
    call(
        &tool,
        json!({"action": "resize", "width": 1100, "height": 760}),
    )
    .unwrap();
    assert_eq!(
        *window.asked.lock().unwrap(),
        [
            format!("opened {}", tabs[0]),
            format!("opened {second}"),
            format!("Show {second}"),
            "resize 1100×760".to_string()
        ]
    );
}

#[test]
fn a_screenshot_is_the_kinds_own() {
    let web = Fake::new("browser", 'w');
    let tool = WindowTool {
        kinds: vec![&web],
        window: None,
        hands: None,
        gate: &Open,
    };
    call(&tool, json!({"action": "open", "kind": "browser"})).unwrap();
    let shot = call(&tool, json!({"action": "screenshot", "tab": "w1"})).unwrap();
    assert_eq!(shot.report, "a picture");
}

#[test]
fn list_reads_the_window_where_it_shows_else_every_kinds_tabs() {
    let term = Fake::new("terminal", 't');
    let window = Shown::default();
    let shown = WindowTool {
        kinds: vec![&term],
        window: Some(&window),
        hands: None,
        gate: &Open,
    };
    let listed = call(&shown, json!({"action": "list"})).unwrap();
    assert_eq!(
        listed.report,
        "1 tab:\nw1  browser  Example — https://example.org/  (shown, the browser acts here)"
    );
    // With another project's tabs in the window, each tab names its own.
    let window = Shown {
        others: vec![WindowTab {
            id: "t4".into(),
            kind: "terminal".into(),
            title: "make".into(),
            detail: String::new(),
            shown: false,
            acted_on: false,
            group: "shop".into(),
        }],
        ..Shown::default()
    };
    let shared = WindowTool {
        window: Some(&window),
        ..shown
    };
    assert_eq!(
        call(&shared, json!({"action": "list"})).unwrap().report,
        "2 tabs:\nw1  browser  Example — https://example.org/  (group llx, shown, the browser \
         acts here)\nt4  terminal  make  (group shop)"
    );
    let hidden = WindowTool {
        kinds: vec![&term],
        window: None,
        hands: None,
        gate: &Open,
    };
    assert_eq!(
        call(&hidden, json!({"action": "list"})).unwrap().report,
        "No tabs are open."
    );
    call(
        &hidden,
        json!({"action": "open", "kind": "terminal", "thing": "sh"}),
    )
    .unwrap();
    assert_eq!(
        call(&hidden, json!({"action": "list"})).unwrap().report,
        "1 tab:\nt1  terminal  terminal 1"
    );
}

#[test]
fn a_wrong_call_says_why() {
    let term = Fake::new("terminal", 't');
    let tool = WindowTool {
        kinds: vec![&term],
        window: None,
        hands: None,
        gate: &Open,
    };
    for (arguments, said) in [
        (json!({"action": "fly"}), "`action` is one of"),
        (json!({"action": "open"}), "`kind` is one of: terminal"),
        (
            json!({"action": "open", "kind": "spaceship"}),
            "`kind` is one of: terminal",
        ),
        (json!({"action": "interact"}), "`tab` is needed"),
        (json!({"action": "interact", "tab": "x9"}), "no tab x9"),
        (
            json!({"action": "interact", "tab": "t9"}),
            "no terminal tab 9",
        ),
        (
            json!({"action": "show", "tab": "t1"}),
            "The window is not shown",
        ),
        (
            json!({"action": "resize", "width": 900, "height": 700}),
            "The window is not shown",
        ),
    ] {
        match call(&tool, arguments.clone()) {
            Err(ToolError::Failed(why)) => assert!(why.contains(said), "{arguments}: {why}"),
            other => panic!("{arguments}: {other:?}"),
        }
    }
}

#[test]
fn the_schema_holds_every_kinds_fields_and_guide() {
    let (web, term) = (Fake::new("browser", 'w'), Fake::new("terminal", 't'));
    let tool = WindowTool {
        kinds: vec![&web, &term],
        window: None,
        hands: None,
        gate: &Open,
    };
    let spec = tool.spec();
    assert_eq!(spec.name, "window");
    assert!(
        spec.description
            .contains("browser (`w` tabs): how to use it"),
        "{}",
        spec.description
    );
    assert_eq!(
        spec.parameters["properties"]["kind"]["enum"],
        json!(["browser", "terminal"])
    );
    assert!(spec.parameters["properties"]["thing"].is_object());
}

#[test]
fn a_goal_works_on_any_kind_of_tab_through_its_view() {
    let term = Fake::new("terminal", 't');
    let (hands, gate) = (Helper::default(), Recorder::default());
    let tool = WindowTool {
        kinds: vec![&term],
        window: None,
        hands: Some(&hands),
        gate: &gate,
    };
    call(
        &tool,
        json!({"action": "open", "kind": "terminal", "thing": "claude"}),
    )
    .unwrap();
    let done = call(
        &tool,
        json!({"action": "interact", "tab": "t1", "goal": "Say hi; done when it answers",
            "values": ["hi", "bye"]}),
    )
    .unwrap();
    assert_eq!(
        *hands.saw.lock().unwrap(),
        ["terminal 1: Say hi; done when it answers"]
    );
    assert_eq!(
        *gate.asked.lock().unwrap(),
        ["terminal: goal on t1: Say hi; done when it answers (may type \"hi\", \"bye\")"]
    );
    assert!(
        done.report
            .starts_with("t1: ### Helper: done (90% sure)\n1. TYPE"),
        "{}",
        done.report
    );
    assert!(
        done.report.contains("Values never typed: \"bye\"."),
        "{}",
        done.report
    );
    assert_eq!(
        done.state.as_deref(),
        Some("snapshot of 1 after"),
        "as the tab says after an action"
    );
    // Refused, nothing is handed over.
    let refused = WindowTool {
        kinds: vec![&term],
        window: None,
        hands: Some(&hands),
        gate: &Recorder::denying("not now"),
    };
    assert!(
        call(
            &refused,
            json!({"action": "interact", "tab": "t1", "goal": "x"})
        )
        .is_err()
    );
    assert_eq!(hands.saw.lock().unwrap().len(), 1);
}

/// A kind whose tabs open only together: each open waits (2 s at most) for the other to start.
struct Together(std::sync::atomic::AtomicUsize);

impl Kind for Together {
    fn name(&self) -> &'static str {
        "slow"
    }
    fn letter(&self) -> char {
        's'
    }
    fn guide(&self) -> &'static str {
        ""
    }
    fn fields(&self) -> Vec<(&'static str, Value)> {
        vec![]
    }
    #[allow(
        clippy::disallowed_methods,
        reason = "a test's fake waits, bounded, for the other tab to open beside it"
    )]
    fn open(&self, _: &Value) -> Result<Opened, ToolError> {
        use std::sync::atomic::Ordering;
        self.0.fetch_add(1, Ordering::SeqCst);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while self.0.load(Ordering::SeqCst) < 2 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        match self.0.load(Ordering::SeqCst) >= 2 {
            true => Ok(Opened {
                id: 1,
                output: "met".to_string().into(),
            }),
            false => Err(ToolError::Failed("opened alone".into())),
        }
    }
    #[allow(
        clippy::panic_in_result_fn,
        reason = "a test's fake: a call it should not get fails the test"
    )]
    fn interact(&self, _: u32, _: &Value) -> Result<ToolOutput, ToolError> {
        panic!("not called here")
    }
    #[allow(
        clippy::panic_in_result_fn,
        reason = "a test's fake: a call it should not get fails the test"
    )]
    fn snapshot(&self, _: u32) -> Result<ToolOutput, ToolError> {
        panic!("not called here")
    }
    #[allow(
        clippy::panic_in_result_fn,
        reason = "a test's fake: a call it should not get fails the test"
    )]
    fn screenshot(&self, _: u32, _: &Value) -> Result<ToolOutput, ToolError> {
        panic!("not called here")
    }
    #[allow(
        clippy::panic_in_result_fn,
        reason = "a test's fake: a call it should not get fails the test"
    )]
    fn close(&self, _: u32) -> Result<String, ToolError> {
        panic!("not called here")
    }
    fn tabs(&self) -> Vec<(u32, String, String)> {
        vec![]
    }
    #[allow(
        clippy::panic_in_result_fn,
        reason = "a test's fake: a call it should not get fails the test"
    )]
    fn view(&self, _: u32) -> Result<Box<dyn View + '_>, ToolError> {
        panic!("not called here")
    }
}

#[test]
fn the_tabs_of_one_call_open_at_the_same_time_and_calls_may_run_together() {
    let kind = Together(std::sync::atomic::AtomicUsize::new(0));
    let tool = WindowTool {
        kinds: vec![&kind],
        window: None,
        hands: None,
        gate: &Open,
    };
    assert!(tool.parallel(), "a turn's window calls run at once");
    let done = call(
        &tool,
        json!({"action": "open", "tabs": [{"kind": "slow"}, {"kind": "slow"}]}),
    )
    .unwrap();
    assert_eq!(done.report, "s1 (slow): met\ns1 (slow): met");
}

#[test]
fn a_tabs_own_fields_come_before_its_goal_and_the_tab_is_read_after() {
    let term = Fake::new("terminal", 't');
    let (hands, gate) = (Helper::default(), Recorder::default());
    let tool = WindowTool {
        kinds: vec![&term],
        window: None,
        hands: Some(&hands),
        gate: &gate,
    };
    call(
        &tool,
        json!({"action": "open", "kind": "terminal", "thing": "sh"}),
    )
    .unwrap();
    call(
        &tool,
        json!({"action": "interact", "tab": "t1", "thing": "go there", "goal": "Act", "values": ["x"]}),
    )
    .unwrap();
    // The address first (the goal acts on the page it loads), then the goal, then a read.
    assert_eq!(
        *term.done.lock().unwrap(),
        ["open \"sh\"", "interact 1 \"go there\"", "interact 1 null"]
    );
}
