use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

use protocol::{Command, Done, Place, ToClient, ToServer, ToolInfo};
use serde_json::{Value, json};

use super::*;

/// A server that answers as a toolbox does: a call of `slow` waits for its `Halt`, any other
/// says what it was given. What it was sent is kept.
#[allow(
    clippy::disallowed_methods,
    reason = "the in-process transport's channels, as the binary makes them"
)]
fn server() -> (Client, Arc<Mutex<Vec<Command>>>) {
    let (to_server, incoming) = channel::<ToServer>();
    let (out, from_server) = channel::<ToClient>();
    let sent = Arc::new(Mutex::new(vec![]));
    let kept = Arc::clone(&sent);
    std::thread::spawn(move || toolbox(&incoming, &out, &kept));
    let client = Client::in_process(to_server, from_server, |_| {}).unwrap();
    (client, sent)
}

fn toolbox(
    incoming: &Receiver<ToServer>,
    out: &std::sync::mpsc::Sender<ToClient>,
    sent: &Mutex<Vec<Command>>,
) {
    let mut slow = vec![];
    for frame in incoming {
        let ToServer::Call { id, command } = frame else {
            continue;
        };
        sent.lock().unwrap().push(command.clone());
        let done = match command {
            Command::Initialize { .. } => Done::Ok,
            Command::Tools => Done::Tools {
                tools: vec![ToolInfo {
                    name: "window".into(),
                    description: "Tabs.".into(),
                    parameters: json!({"type": "object"}),
                }],
            },
            Command::Tool {
                call, arguments, ..
            } if arguments["action"] == "slow" => {
                slow.push((id, call));
                continue;
            }
            Command::Tool { arguments, .. } => Done::Tool {
                report: format!("did {}", arguments["action"]),
                state: None,
                image: None,
            },
            Command::Halt { id: halted } => {
                for (call_id, _) in slow.iter().filter(|(_, call)| *call == halted) {
                    let why = "stopped".to_string();
                    let done = Done::Failed { why };
                    out.send(ToClient::Done { id: *call_id, done }).unwrap();
                }
                Done::Ok
            }
            _ => Done::Failed { why: "no".into() },
        };
        out.send(ToClient::Done { id, done }).unwrap();
    }
}

/// An agent at `run`'s stdin and stdout, as a process would be: its stdin open until it
/// leaves, each answer read as it comes.
struct Agent {
    stdin: Option<io::PipeWriter>,
    answers: Receiver<Value>,
    read: Vec<Value>,
    running: Option<std::thread::JoinHandle<io::Result<()>>>,
}

impl Agent {
    #[allow(
        clippy::disallowed_methods,
        reason = "what the agent reads, as a process's stdout would carry it"
    )]
    fn start(client: Client) -> Self {
        let (input, stdin) = io::pipe().unwrap();
        let (stdout, output) = io::pipe().unwrap();
        let place = Place {
            args: vec!["mcp".into()],
            ..Place::default()
        };
        let running = std::thread::spawn(move || {
            run(
                &client,
                place,
                String::new(),
                io::BufReader::new(input),
                output,
            )
        });
        let (each, answers) = channel();
        std::thread::spawn(move || {
            use io::BufRead;
            for line in io::BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = each.send(serde_json::from_str(&line).unwrap());
            }
        });
        Self {
            stdin: Some(stdin),
            answers,
            read: vec![],
            running: Some(running),
        }
    }

    fn send(&mut self, line: &str) {
        use io::Write;
        writeln!(self.stdin.as_mut().unwrap(), "{line}").unwrap();
    }

    fn ask(&mut self, message: Value) {
        self.send(&message.to_string());
    }

    /// The answer to request `id`, when it comes.
    fn answer(&mut self, id: Value) -> Value {
        if let Some(at) = self.read.iter().position(|answer| answer["id"] == id) {
            return self.read.remove(at);
        }
        loop {
            let answer = self.answers.recv_timeout(WAIT).expect("no answer");
            if answer["id"] == id {
                return answer;
            }
            self.read.push(answer);
        }
    }

    /// The agent leaves: what it was still told.
    fn close(mut self) -> Vec<Value> {
        drop(self.stdin.take());
        self.running.take().unwrap().join().unwrap().unwrap();
        self.read.extend(self.answers.iter());
        self.read
    }
}

const WAIT: Duration = Duration::from_secs(5);

/// Whether the server was sent a command `matching`, within a wait.
#[allow(
    clippy::disallowed_methods,
    reason = "what the fake server saw has no event: it is looked at every 10 ms, bounded"
)]
fn saw(sent: &Mutex<Vec<Command>>, matching: impl Fn(&Command) -> bool) -> bool {
    let end = std::time::Instant::now() + WAIT;
    while std::time::Instant::now() < end {
        if sent.lock().unwrap().iter().any(&matching) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn another_agent_lists_the_window_and_calls_it() {
    let (client, sent) = server();
    let mut agent = Agent::start(client);
    agent.ask(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {}}}));
    agent.ask(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    agent.ask(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    agent.ask(json!({"jsonrpc": "2.0", "id": "c", "method": "tools/call",
        "params": {"name": "window", "arguments": {"action": "list"}}}));
    let init = agent.answer(json!(1));
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    let listed = agent.answer(json!(2));
    assert_eq!(listed["result"]["tools"][0]["name"], "window");
    assert_eq!(
        listed["result"]["tools"][0]["inputSchema"],
        json!({"type": "object"})
    );
    let called = agent.answer(json!("c"));
    assert_eq!(called["result"]["content"][0]["text"], "did \"list\"");
    assert_eq!(agent.close(), Vec::<Value>::new());
    // The handshake said where it is, as a tool client.
    let sent = sent.lock().unwrap();
    assert!(
        matches!(&sent[0], Command::Initialize { place: Some(place), .. }
        if place.args == ["mcp"])
    );
}

#[test]
fn a_cancelled_call_is_halted_by_its_id_and_never_answered() {
    let (client, sent) = server();
    let mut agent = Agent::start(client);
    agent.ask(json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": {"name": "window", "arguments": {"action": "slow"}}}));
    agent.ask(
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled",
        "params": {"requestId": 7}}),
    );
    // Halted while the agent is still here, by the id the call was sent with.
    let call = |command: &Command| match command {
        Command::Tool { call, .. } => Some(call.clone()),
        _ => None,
    };
    assert!(saw(&sent, |command| matches!(
        command,
        Command::Halt { .. }
    )));
    let halted = sent.lock().unwrap().iter().find_map(call).unwrap();
    assert!(sent.lock().unwrap().contains(&Command::Halt { id: halted }));
    agent.ask(json!({"jsonrpc": "2.0", "id": 8, "method": "ping"}));
    agent.answer(json!(8));
    assert_eq!(agent.close(), Vec::<Value>::new(), "7 is never answered");
}

#[test]
fn a_call_still_running_when_the_agent_leaves_is_halted() {
    let (client, sent) = server();
    let mut agent = Agent::start(client);
    agent.ask(json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": {"name": "window", "arguments": {"action": "slow"}}}));
    assert!(saw(&sent, |command| matches!(
        command,
        Command::Tool { .. }
    )));
    assert_eq!(agent.close(), Vec::<Value>::new(), "nobody reads it");
    assert!(saw(&sent, |command| matches!(
        command,
        Command::Halt { .. }
    )));
}

#[test]
fn what_is_not_known_is_refused_and_a_notification_gets_nothing() {
    let (client, _) = server();
    let mut agent = Agent::start(client);
    agent.ask(json!({"jsonrpc": "2.0", "id": 1, "method": "resources/list"}));
    agent.ask(json!({"jsonrpc": "2.0", "method": "notifications/whatever"}));
    agent.ask(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {}}));
    agent.ask(json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}));
    assert_eq!(agent.answer(json!(1))["error"]["code"], -32601);
    assert_eq!(agent.answer(json!(2))["error"]["code"], -32602);
    assert_eq!(agent.answer(json!(3))["result"], json!({}));
    assert_eq!(agent.close(), Vec::<Value>::new());
}

#[test]
fn a_line_that_is_not_a_message_is_a_parse_error() {
    let (client, _) = server();
    let mut agent = Agent::start(client);
    agent.send("{oops");
    let answer = agent.answer(Value::Null);
    assert_eq!(answer["error"]["code"], -32700);
    agent.close();
}
