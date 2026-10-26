use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use protocol::Place;
use serde_json::json;

use super::*;
use crate::testing::Scratch;

const WAIT: Duration = Duration::from_secs(10);

/// A tool client's connection to a toolbox on a thread of its own, at `scratch`.
struct Connected {
    to_server: Option<Sender<ToServer>>,
    out: Receiver<ToClient>,
    terminals: Arc<AtomicU32>,
    served: Option<std::thread::JoinHandle<()>>,
}

fn connected(scratch: &Scratch) -> Connected {
    owned(scratch, &Arc::default())
}

/// As [`connected`], for owner `me` of `owners`.
#[expect(
    clippy::disallowed_methods,
    reason = "the in-process transport's channels, as the socket's connection makes them"
)]
fn owned(scratch: &Scratch, owners: &Arc<Owners>) -> Connected {
    let (to_server, incoming) = channel();
    let (out, from_server) = channel();
    let terminals: Arc<AtomicU32> = Arc::default();
    let hosted = Hosted {
        host: None,
        browser: None,
        lanes: crate::arrivals::listening(None),
        terminals: Arc::clone(&terminals),
        owners: Arc::clone(owners),
    };
    let first = ToServer::Call {
        id: 1,
        command: Command::Initialize {
            version: protocol::VERSION,
            place: Some(Place {
                cwd: scratch.path().display().to_string(),
                env: std::env::vars().collect(),
                args: vec!["window".into(), "me".into()],
                ..Place::default()
            }),
            build: String::new(),
        },
    };
    let served = std::thread::spawn(move || serve(first, &incoming, &out, &hosted));
    let connected = Connected {
        to_server: Some(to_server),
        out: from_server,
        terminals,
        served: Some(served),
    };
    let initialized = connected.done(1);
    assert!(
        matches!(initialized, Done::Initialized { .. }),
        "{initialized:?}"
    );
    connected
}

impl Connected {
    fn call(&self, id: u64, command: Command) {
        let frame = ToServer::Call { id, command };
        self.to_server.as_ref().unwrap().send(frame).unwrap();
    }

    /// What command `id` came to: the answers come as their calls end, in any order.
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "a test picks out the variant it checks"
    )]
    fn done(&self, id: u64) -> Done {
        loop {
            match self.out.recv_timeout(WAIT).unwrap() {
                ToClient::Done { id: done_id, done } if done_id == id => return done,
                _ => {}
            }
        }
    }

    fn window(&self, id: u64, call: &str, arguments: serde_json::Value) {
        let (call, name) = (call.into(), "window".into());
        self.call(
            id,
            Command::Tool {
                call,
                name,
                arguments,
            },
        );
    }

    /// The connection ends, and the toolbox with it.
    fn end(mut self) -> Arc<AtomicU32> {
        drop(self.to_server.take());
        self.served.take().unwrap().join().unwrap();
        Arc::clone(&self.terminals)
    }
}

/// What a tool's call reported, its state after.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "a test picks out the variant it checks"
)]
fn told(done: Done) -> String {
    match done {
        Done::Tool { report, state, .. } => format!("{report}\n\n{}", state.unwrap_or_default()),
        other => panic!("not a tool's result: {other:?}"),
    }
}

#[test]
fn a_tool_client_is_offered_the_window_and_its_kinds() {
    let scratch = Scratch::new();
    let client = connected(&scratch);
    client.call(2, Command::Tools);
    let Done::Tools { tools } = client.done(2) else {
        panic!("no tools");
    };
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
    assert_eq!(names, ["window"]);
    assert!(tools[0].description.contains("terminal"), "{tools:?}");
    assert_eq!(tools[0].parameters["required"], json!(["action"]));
}

#[test]
fn an_owner_s_tabs_outlive_its_connection_until_it_releases_them() {
    let scratch = Scratch::new();
    let owners: Arc<Owners> = Arc::default();
    let first = owned(&scratch, &owners);
    let open = json!({"action": "open", "kind": "terminal", "command": "cat"});
    first.window(2, "c1", open);
    assert!(told(first.done(2)).contains("t1"));
    let terminals = first.end();
    assert_eq!(tools::shell::running_with(&terminals).len(), 1);
    assert_eq!(owners.holding(), 1);
    // The next connection of the owner finds its tab.
    let next = owned(&scratch, &owners);
    let typed = json!({"action": "interact", "tab": "t1", "input": "again\n", "wait_for": "again"});
    next.window(2, "c2", typed);
    assert!(told(next.done(2)).contains("again"));
    next.call(3, Command::Release);
    assert_eq!(next.done(3), Done::Ok);
    assert!(tools::shell::running_with(&terminals).is_empty());
    assert_eq!(owners.holding(), 0);
    next.end();
}

#[test]
fn a_terminal_tab_runs_where_the_client_is() {
    let scratch = Scratch::new();
    let client = connected(&scratch);
    let open = json!({"action": "open", "kind": "terminal", "command": "pwd; cat"});
    client.window(2, "c1", open);
    let opened = told(client.done(2));
    assert!(opened.contains("t1"), "{opened}");
    assert!(
        opened.contains(&scratch.path().display().to_string()),
        "{opened}"
    );
    let typed = json!({"action": "interact", "tab": "t1", "input": "hello\n", "wait_for": "hello"});
    client.window(3, "c2", typed);
    assert!(told(client.done(3)).contains("hello"));
    client.call(4, Command::Release);
    assert_eq!(client.done(4), Done::Ok);
    let terminals = client.end();
    assert!(tools::shell::running_with(&terminals).is_empty());
}

#[test]
fn a_call_halted_by_its_id_stops_while_the_others_are_served() {
    let scratch = Scratch::new();
    let client = connected(&scratch);
    let waits = json!({"action": "open", "kind": "terminal", "command": "sleep 30",
        "wait_for": "never shown", "yield_ms": 30000});
    let started = Instant::now();
    client.window(2, "slow", waits);
    // Served while the first runs: the connection is not held by a call.
    client.call(3, Command::Tools);
    assert!(matches!(client.done(3), Done::Tools { .. }));
    client.call(4, Command::Halt { id: "slow".into() });
    assert_eq!(client.done(4), Done::Ok);
    let _ = client.done(2);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "it was not stopped"
    );
    client.end();
}

#[test]
fn a_tool_that_is_not_there_fails_naming_the_one_there_is() {
    let scratch = Scratch::new();
    let client = connected(&scratch);
    let (call, name, arguments) = ("c1".into(), "bash".into(), json!({"command": "ls"}));
    client.call(
        2,
        Command::Tool {
            call,
            name,
            arguments,
        },
    );
    let Done::Failed { why } = client.done(2) else {
        panic!("it did not fail");
    };
    assert!(why.contains("window"), "{why}");
}

#[test]
fn a_tool_client_opens_no_session() {
    let scratch = Scratch::new();
    let client = connected(&scratch);
    client.call(2, Command::List);
    assert!(matches!(client.done(2), Done::Failed { .. }));
    let refused = Command::Open {
        target: protocol::Target::New,
    };
    client.call(3, refused);
    assert!(matches!(client.done(3), Done::Failed { .. }));
}

/// A daemon with no window to give (no display, or the headless build, ARCHITECTURE.md A8): a page
/// cannot open, and the client reads why; its terminal tabs go on working.
#[test]
fn where_there_is_no_window_a_page_fails_with_the_reason_and_a_terminal_still_opens() {
    let scratch = Scratch::new();
    let client = connected(&scratch);
    let page = json!({"action": "open", "kind": "browser", "url": "https://example.com"});
    client.window(2, "c1", page);
    let refused = told(client.done(2));
    assert!(
        refused.contains("not opened") && refused.contains(tools::browser::NO_WINDOW),
        "{refused}"
    );
    let terminal = json!({"action": "open", "kind": "terminal", "command": "echo still here; cat"});
    client.window(3, "c2", terminal);
    let opened = told(client.done(3));
    assert!(
        opened.contains("t1") && opened.contains("still here"),
        "{opened}"
    );
    client.end();
}
