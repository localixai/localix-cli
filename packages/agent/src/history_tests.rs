use contracts::chat::{Message, ToolCall};
use contracts::tool::ToolOutput;
use serde_json::json;

use super::*;

/// A conversation of `reads` calls, each reading `path-N` (or the same path when `same`) and
/// getting `chars` characters back.
fn reading(reads: usize, chars: usize, same: bool) -> Conversation {
    let mut conversation = Conversation::new("rules");
    conversation.push(Message::User("Goal: look".into()));
    for n in 0..reads {
        let path = if same {
            "a.rs".to_string()
        } else {
            format!("f{n}.rs")
        };
        conversation.push(Message::Assistant {
            text: None,
            calls: vec![ToolCall {
                id: format!("c{n}"),
                name: "look".into(),
                arguments: json!({"path": path}),
            }],
        });
        conversation.result(
            &format!("c{n}"),
            ToolOutput {
                report: "x".repeat(chars),
                ..ToolOutput::default()
            },
        );
    }
    conversation
}

fn contents(conversation: &Conversation) -> Vec<String> {
    conversation
        .messages()
        .iter()
        .filter_map(|message| match message {
            Message::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect()
}

fn elided(conversation: &Conversation) -> Vec<bool> {
    contents(conversation)
        .iter()
        .map(|content| content.contains(MARKER))
        .collect()
}

#[test]
fn the_ceiling_leaves_a_reserve_and_stays_under_auto() {
    let pipeline = Pipeline::default();
    assert_eq!(
        pipeline.ceiling(0),
        128_000 - 16_384,
        "unknown: 128k assumed"
    );
    assert_eq!(
        pipeline.ceiling(32_000),
        32_000 - 6_400,
        "small: 20% reserved"
    );
    assert_eq!(
        pipeline.ceiling(200_000),
        200_000 - 20_000,
        "10% above 160k"
    );
    assert_eq!(
        pipeline.ceiling(1_000_000),
        272_000,
        "auto caps a long window"
    );
}

#[test]
fn under_the_gate_nothing_changes_for_the_cache() {
    let mut conversation = reading(10, 4_000, false);
    let before = contents(&conversation);
    // About 10k tokens against a 20k ceiling: under 90%.
    let report = Pipeline::default().prepare(&mut conversation, 20_000, 1.0);
    assert!(report.is_empty());
    assert_eq!(contents(&conversation), before);
}

#[test]
fn past_the_gate_old_outputs_go_down_to_the_target_and_the_tail_stays() {
    let pipeline = Pipeline {
        tail: 2_000,
        ..Pipeline::default()
    };
    let mut conversation = reading(10, 4_000, false);
    let report = pipeline.prepare(&mut conversation, 11_000, 1.0);
    assert_eq!(report, [("outputs", 4)]);
    // The oldest go first, only as many as the target needs; the newest stay whole.
    let marks = elided(&conversation);
    assert!(
        marks[..4].iter().all(|e| *e) && marks[4..].iter().all(|e| !*e),
        "{marks:?}"
    );
    assert!(
        Frame {
            ceiling: 11_000,
            target: 6_600,
            ratio: 1.0,
            tail: 0
        }
        .size(&conversation)
            <= 6_600
    );
}

#[test]
fn superseded_outputs_go_before_any_other() {
    let pipeline = Pipeline {
        tail: 1_000,
        ..Pipeline::default()
    };
    let mut conversation = reading(6, 4_000, true);
    let report = pipeline.prepare(&mut conversation, 6_000, 1.0);
    assert_eq!(report[0].0, "superseded");
    assert!(contents(&conversation)[0].starts_with("[output elided to save context: 4000"));
}

#[test]
fn what_was_elided_stays_elided_and_a_second_pass_changes_nothing() {
    let pipeline = Pipeline {
        tail: 2_000,
        ..Pipeline::default()
    };
    let mut conversation = reading(10, 4_000, false);
    pipeline.prepare(&mut conversation, 11_000, 1.0);
    let after = contents(&conversation);
    let report = pipeline.prepare(&mut conversation, 11_000, 1.0);
    assert!(report.is_empty(), "under the gate now: {report:?}");
    assert_eq!(
        contents(&conversation),
        after,
        "the prefix is the same for the cache"
    );
}

#[test]
fn large_arguments_go_when_outputs_are_not_enough() {
    let mut conversation = Conversation::new("rules");
    conversation.push(Message::User("Goal: write".into()));
    for n in 0..6 {
        conversation.push(Message::Assistant {
            text: None,
            calls: vec![ToolCall {
                id: format!("w{n}"),
                name: "write".into(),
                arguments: json!({"path": format!("f{n}"), "text": "y".repeat(4_000)}),
            }],
        });
        conversation.result(&format!("w{n}"), ToolOutput::default());
    }
    let pipeline = Pipeline {
        tail: 1_000,
        ..Pipeline::default()
    };
    let report = pipeline.prepare(&mut conversation, 6_000, 1.0);
    assert_eq!(report[0].0, "arguments");
    let first = &conversation.messages()[2];
    assert!(matches!(first, Message::Assistant { calls, .. }
            if calls[0].arguments["text"] == json!("[elided to save context: 4000 characters]")
            && calls[0].arguments["path"] == json!("f0")));
}

#[test]
fn the_real_size_counts() {
    // The estimate is 10k, under the gate of a 21k ceiling...
    let mut estimated = reading(10, 4_000, false);
    let report = Pipeline::default().prepare(&mut estimated, 21_000, 1.0);
    assert!(report.is_empty(), "{report:?}");
    // ...but the model said the real size is twice that: the oldest outputs go.
    let mut conversation = reading(10, 4_000, false);
    let report = Pipeline::default().prepare(&mut conversation, 21_000, 2.0);
    let marks = elided(&conversation);
    let gone = marks.iter().filter(|e| **e).count();
    assert_eq!(report, [("outputs", gone)], "{marks:?}");
    assert!(gone > 0, "{marks:?}");
    assert!(
        marks[..gone].iter().all(|e| *e) && !marks[9],
        "the oldest went, the newest stays: {marks:?}"
    );
}

#[test]
fn past_the_limit_itself_the_tail_gives_way_but_its_last_messages_stay() {
    let mut conversation = reading(10, 4_000, false);
    // The default tail (16k tokens) covers the whole conversation; the limit is far below it.
    let report = Pipeline::default().prepare(&mut conversation, 6_000, 1.0);
    assert!(!report.is_empty());
    let marks = elided(&conversation);
    assert!(marks[0] && !marks[9] && !marks[8], "{marks:?}");
}

/// A long dialogue through the agent's own loop: a model that reads files (some again) and
/// writes some, against a small window, each request recorded and checked.
mod dialogue {
    use std::sync::Mutex;

    use contracts::chat::{Chat, ChatError, Completion, Request, Usage};
    use contracts::tool::{Tool, ToolError, ToolSpec};
    use serde_json::Value;

    use super::*;
    use crate::agent::Agent;
    use crate::observer::Silent;

    const TURNS: usize = 300;
    /// Real tokens per estimated one, as the model reports them.
    const REAL: f64 = 1.3;
    const WINDOW: u64 = 60_000;

    /// A model that calls `look` (or `write`) for `TURNS` turns, then answers; it reports the
    /// real size of each request and keeps a copy of it.
    #[derive(Default)]
    struct Model {
        requests: Mutex<Vec<Vec<Message>>>,
    }

    impl Chat for Model {
        #[expect(
            clippy::unwrap_in_result,
            reason = "a test's fake: a poisoned lock fails the test"
        )]
        fn complete(&self, request: Request) -> Result<Completion, ChatError> {
            let mut requests = self.requests.lock().unwrap();
            let turn = requests.len();
            requests.push(request.conversation.to_vec());
            let estimate: u64 = request.conversation.iter().map(tokens).sum();
            let usage = Usage {
                // The specs go with every request, and count.
                input_tokens: whole((estimate + specs(request.tools)) as f64 * REAL),
                ..Usage::default()
            };
            if turn >= TURNS {
                return Ok(Completion {
                    text: Some("Done.\nRESULT: achieved".into()),
                    usage,
                    ..Completion::default()
                });
            }
            // A file of 37 read in turn: some are read again, which supersedes the earlier read.
            let path = format!("src/f{}.rs", turn * 7 % 37);
            let call = match turn % 7 {
                6 => ToolCall {
                    id: format!("t{turn}"),
                    name: "write".into(),
                    arguments: json!({"path": path, "text": "w".repeat(3_000)}),
                },
                _ => ToolCall {
                    id: format!("t{turn}"),
                    name: "look".into(),
                    arguments: json!({"path": path}),
                },
            };
            Ok(Completion {
                calls: vec![call],
                usage,
                ..Completion::default()
            })
        }
    }

    fn specs(specs: &[ToolSpec]) -> u64 {
        let size = |spec: &ToolSpec| {
            spec.name.len() + spec.description.len() + spec.parameters.to_string().len()
        };
        specs.iter().map(|spec| size(spec) as u64 / 4).sum()
    }

    /// Reads give 3,000 to 6,000 characters; writes say they wrote. Each is described at length,
    /// as real tools are.
    struct Files(&'static str);

    impl Tool for Files {
        fn spec(&self) -> ToolSpec {
            ToolSpec {
                name: self.0.into(),
                description: "d".repeat(8_000),
                parameters: json!({"type": "object"}),
            }
        }

        fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
            let path = arguments["path"].as_str().unwrap_or("");
            let report = match self.0 {
                "write" => format!("Wrote {path}."),
                _ => format!("{path}\n{}", "r".repeat(3_000 + path.len() * 97 % 3_000)),
            };
            Ok(ToolOutput {
                report,
                ..ToolOutput::default()
            })
        }
    }

    fn id_of(message: &Message) -> Option<(&str, bool)> {
        match message {
            Message::ToolResult { id, content, .. } => Some((id, content.contains(MARKER))),
            _ => None,
        }
    }

    #[test]
    fn a_long_dialogue_stays_within_the_limit_and_keeps_the_cache() {
        let model = Model::default();
        let (look, write) = (Files("look"), Files("write"));
        let tools: [&dyn Tool; 2] = [&look, &write];
        let pipeline = Pipeline::default();
        let agent = Agent {
            chat: &model,
            tools: &tools,
            context: "Working directory: /work",
            max_turns: TURNS + 5,
            observer: &Silent,
            schema: None,
            inbox: None,
            window: WINDOW,
            strategy: &pipeline,
        };
        let run = agent.run("read the project");
        assert_eq!(run.answer, "Done.");
        let requests = model.requests.into_inner().unwrap();
        assert_eq!(requests.len(), TURNS + 1);
        let ceiling = pipeline.ceiling(WINDOW);
        let mut breaks = 0;
        let mut peak = 0;
        let mut ever_elided = HashSet::new();
        let fixed = specs(&[look.spec(), write.spec()]);
        for (at, request) in requests.iter().enumerate() {
            // It fits: the real size is under the model's limit, always.
            let real = whole((request.iter().map(tokens).sum::<u64>() + fixed) as f64 * REAL);
            assert!(real <= ceiling, "request {at}: {real} > {ceiling}");
            peak = peak.max(real);
            // Nothing is dropped: two messages a turn, after the rules, the context and the goal.
            assert_eq!(request.len(), 3 + 2 * at);
            assert_eq!(request[2], Message::User("Goal: read the project".into()));
            // The tail is whole: the newest results are never elided.
            let last: Vec<_> = request.iter().rev().filter_map(id_of).take(2).collect();
            assert!(last.iter().all(|(_, elided)| !elided), "request {at}");
            // The ratchet: what was elided once stays elided.
            for (id, elided) in request.iter().filter_map(id_of) {
                if ever_elided.contains(id) {
                    assert!(elided, "request {at}: {id} came back");
                }
                if elided {
                    ever_elided.insert(id.to_string());
                }
            }
            // The cache: each request starts with the one before it, but at a pass.
            if at > 0 {
                let before = &requests[at - 1];
                if request[..before.len()] != before[..] {
                    breaks += 1;
                }
            }
        }
        // The window moved: the oldest are elided, the newest are whole.
        let last = requests.last().unwrap();
        let marks: Vec<bool> = last.iter().filter_map(id_of).map(|(_, e)| e).collect();
        assert!(marks[..TURNS / 2].iter().filter(|e| **e).count() > TURNS / 3);
        assert!(!marks[TURNS - 3..].iter().any(|e| *e));
        // A pass frees 30% of the limit, so it comes about once every dozen turns.
        assert!(
            breaks > 0 && breaks <= TURNS / 8,
            "{breaks} cache breaks in {TURNS} turns"
        );
        assert!(
            peak * 100 >= ceiling * 85,
            "the limit is used: peak {peak} of {ceiling}"
        );
        println!("{TURNS} turns: {breaks} cache breaks, peak {peak} of {ceiling} tokens");
    }
}
