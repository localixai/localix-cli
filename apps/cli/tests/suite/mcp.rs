//! `llx mcp`: another agent (Claude Code, Codex, Cursor) calls llx's `window` over MCP on stdio.
//! It is the daemon's tool client: the first `llx mcp` of a home starts `llx serve`, and the
//! tabs it opens run where the agent started it and end when the agent closes its stdin.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{Receiver, sync_channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::common::home::{Home, text};

const LONG: Duration = Duration::from_secs(20);

/// `llx mcp` as an agent runs it, in `dir`: its stdin written, its stdout read line by line.
struct Agent {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<Value>,
}

impl Agent {
    fn start(home: &Home, dir: &std::path::Path) -> Self {
        let mut child = home
            .llx(&["mcp"])
            .current_dir(dir)
            // Over SSH no window opens: a test never puts one on the screen.
            .env("SSH_CONNECTION", "10.0.0.1 1 10.0.0.2 22")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (each, lines) = sync_channel(64);
        let reader = std::thread::Builder::new().name("mcp-reader".into());
        reader
            .spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let _ = each.send(serde_json::from_str(&line).unwrap());
                }
            })
            .unwrap();
        let stdin = child.stdin.take();
        Self {
            child,
            stdin,
            lines,
        }
    }

    /// `method` asked with `params` as request `id`, and its answer.
    fn ask(&mut self, id: u64, method: &str, params: Value) -> Value {
        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{message}").unwrap();
        loop {
            let answer = self.lines.recv_timeout(LONG).unwrap();
            if answer["id"] == id {
                return answer;
            }
        }
    }

    /// The text a `window` call with `arguments` came to.
    fn window(&mut self, id: u64, arguments: Value) -> String {
        let params = json!({"name": "window", "arguments": arguments});
        let answer = self.ask(id, "tools/call", params);
        assert_eq!(answer["result"]["isError"], false, "{answer}");
        answer["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string()
    }

    /// The agent closes its stdin: how the process ended, within `LONG`.
    #[allow(
        clippy::disallowed_methods,
        reason = "an exit has no event to wait on here: it is looked at every 20 ms, bounded"
    )]
    fn close(mut self) -> Option<i32> {
        drop(self.stdin.take());
        let end = Instant::now() + LONG;
        while Instant::now() < end {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.code();
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        None
    }
}

#[test]
fn another_agent_works_in_a_terminal_tab_where_it_started_llx_mcp() {
    let home = Home::new();
    let project = home.path().join("project");
    std::fs::create_dir_all(&project).unwrap();
    let project = project.canonicalize().unwrap();
    let mut agent = Agent::start(&home, &project);
    let init = agent.ask(
        1,
        "initialize",
        json!({"protocolVersion": "2025-06-18",
        "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}),
    );
    assert_eq!(init["result"]["serverInfo"]["name"], "llx", "{init}");
    let listed = agent.ask(2, "tools/list", json!({}));
    assert_eq!(listed["result"]["tools"][0]["name"], "window", "{listed}");
    let opened = agent.window(
        3,
        json!({"action": "open", "kind": "terminal", "command": "pwd; cat"}),
    );
    assert!(opened.contains("t1"), "{opened}");
    assert!(opened.contains(&project.display().to_string()), "{opened}");
    let typed = agent.window(
        4,
        json!({"action": "interact", "tab": "t1", "input": "hello mcp\n", "wait_for": "hello mcp"}),
    );
    assert!(typed.contains("hello mcp"), "{typed}");
    let status = text(&home.run(&["serve", "status"]).stdout);
    assert!(status.contains("1 attached"), "{status}");
    assert_eq!(agent.close(), Some(0));
}

#[test]
fn a_call_that_fails_is_a_result_the_agent_s_model_reads() {
    let home = Home::new();
    let mut agent = Agent::start(&home, home.path());
    let params = json!({"name": "window", "arguments": {"action": "interact", "tab": "t9",
        "input": "x"}});
    let answer = agent.ask(1, "tools/call", params);
    assert_eq!(answer["result"]["isError"], true, "{answer}");
    let why = answer["result"]["content"][0]["text"].as_str().unwrap();
    assert!(why.contains("9 is not running"), "{why}");
    assert_eq!(agent.close(), Some(0));
}
