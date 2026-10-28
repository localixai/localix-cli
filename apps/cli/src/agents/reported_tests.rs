use std::sync::atomic::AtomicU64;
use std::time::Instant;

use contracts::tool::{Input, Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::{Value, json};

use super::{Reported, Scene};
use crate::output::Output;
use crate::testing::{Captured, fake_ui};
use crate::tui::app::App;
use crate::tui::test_support::{Term, call, label};

/// A `read` that reads nothing, or refuses.
struct Reads(bool);

impl Tool for Reads {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read".into(),
            description: String::new(),
            input: Input::Json(json!({})),
        }
    }

    /// Reading changes nothing: a script's reads run together.
    fn parallel(&self) -> bool {
        true
    }

    fn call(&self, _: &Value) -> Result<ToolOutput, ToolError> {
        match self.0 {
            true => Ok(ToolOutput::from("text".to_string())),
            false => Err(ToolError::Failed("no such\nfile".into())),
        }
    }
}

#[test]
fn a_call_a_script_makes_shows_under_the_exec_call_running_and_its_failure_too() {
    let (ui, events) = fake_ui();
    let output = Output::with(
        false,
        false,
        Box::new(Captured::default()),
        Box::new(Captured::default()),
    )
    .in_ui(Some(ui));
    let made = AtomicU64::new(0);
    let scene = Scene {
        output: &output,
        who: "s1",
        agent: None,
        made: &made,
    };
    let (good, bad) = (Reads(true), Reads(false));
    let arguments = json!({"path": "a.txt"});
    let ok = Reported {
        inner: &good,
        scene: &scene,
    };
    assert_eq!(ok.call(&arguments).unwrap().report, "text");
    // What may run beside other calls still may, reported: a script's reads run together.
    assert!(ok.parallel());
    let failing = Reported {
        inner: &bad,
        scene: &scene,
    };
    assert!(failing.call(&arguments).is_err());
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    app.event(crate::tui::event::Event::TaskStarted("look".into()), now);
    app.event(
        call("c1", "exec", &label("Running", "Ran", "a script")),
        now,
    );
    for frame in events.try_iter() {
        app.frame(frame, now);
    }
    let screen = Term::new(&app).draw(&mut app, now);
    assert!(screen.contains("exec › Reading a.txt"), "{screen}");
    assert!(
        screen.contains("exec › read failed: no such file"),
        "{screen}"
    );
}

#[test]
fn an_agents_script_names_the_agent() {
    let (ui, events) = fake_ui();
    let output = Output::with(
        false,
        false,
        Box::new(Captured::default()),
        Box::new(Captured::default()),
    )
    .in_ui(Some(ui));
    let made = AtomicU64::new(0);
    let scene = Scene {
        output: &output,
        who: "s2",
        agent: Some("scout"),
        made: &made,
    };
    let good = Reads(true);
    let tool = Reported {
        inner: &good,
        scene: &scene,
    };
    tool.call(&json!({"path": "b.txt"})).unwrap();
    let said = format!("{:?}", events.try_iter().collect::<Vec<_>>());
    assert!(said.contains("scout: exec › Reading b.txt"), "{said}");
}
