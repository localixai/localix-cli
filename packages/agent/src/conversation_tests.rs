use contracts::chat::ToolCall;
use serde_json::json;

use super::*;

fn output(report: &str, state: usize) -> ToolOutput {
    ToolOutput {
        report: report.into(),
        state: Some("x".repeat(state)),
        image: None,
    }
}

#[test]
fn older_states_go_once_the_budget_is_passed_and_their_reports_stay() {
    let mut conversation = Conversation::new("rules");
    let each = MAX_RESULT - 100;
    let fit = BUDGET / each;
    for i in 0..fit {
        conversation.result(&i.to_string(), output(&format!("opened {i}"), each));
    }
    assert!(
        results(&conversation)
            .iter()
            .all(|content| !content.ends_with(OMITTED)),
        "under the budget, all is kept"
    );
    conversation.result("last", output("opened last", each));
    let contents = results(&conversation);
    assert_eq!(contents[0], format!("opened 0\n{OMITTED}"));
    assert!(
        contents[..fit]
            .iter()
            .all(|content| content.ends_with(OMITTED))
    );
    assert!(
        contents[fit].starts_with("opened last\n\nxxx"),
        "the newest keeps its state"
    );
}

fn results(conversation: &Conversation) -> Vec<String> {
    conversation
        .messages()
        .iter()
        .filter_map(|message| match message {
            Message::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_result_too_long_keeps_its_head_and_tail() {
    let text = format!("{}{}", "a".repeat(MAX_RESULT), "b".repeat(10));
    let short = shortened(&text);
    assert!(short.starts_with("aaa") && short.ends_with("bbbbbbbbbb"));
    assert!(
        short.contains("… 10 characters cut …"),
        "{}",
        &short[MAX_RESULT / 2..MAX_RESULT / 2 + 40]
    );
    assert_eq!(shortened("short"), "short");
}

#[test]
fn a_whole_tool_window_comes_through_unshortened() {
    // About what a file window or a command's two streams take at most: 20 KB and a header.
    let window: String = (1..=2000)
        .map(|n| format!("{n:>6}\t{}\n", "x".repeat(2)))
        .collect();
    let report = format!(
        "src/lib.rs (lines 1-2000 of 2000)\n{window}{}",
        "y".repeat(3000)
    );
    assert!(report.chars().count() > 20_000);
    let mut conversation = Conversation::new("rules");
    conversation.result(
        "1",
        ToolOutput {
            report: report.clone(),
            state: None,
            image: None,
        },
    );
    assert_eq!(results(&conversation), [report]);
}

#[test]
fn calls_left_unanswered_are_closed_before_the_conversation_goes_on() {
    let mut conversation = Conversation::new("rules");
    let call = |id: &str| ToolCall {
        id: id.into(),
        name: "echo".into(),
        arguments: json!({}),
    };
    conversation.push(Message::Assistant {
        text: None,
        calls: vec![call("a"), call("b")],
    });
    conversation.failure("a", "refused");
    conversation.close_open_calls();
    conversation.close_open_calls();
    let answered: Vec<&str> = conversation
        .messages()
        .iter()
        .filter_map(|message| match message {
            Message::ToolResult { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(answered, ["a", "b"]);
}

fn shot(n: u32) -> ToolOutput {
    ToolOutput {
        report: format!("shot {n}"),
        state: None,
        image: Some(Image {
            media_type: "image/jpeg".into(),
            data: format!("data{n}"),
            width: n,
            height: 1,
        }),
    }
}

#[test]
fn past_three_pictures_the_older_ones_go_at_once_and_the_prefix_then_stays() {
    let mut conversation = Conversation::new("rules");
    let kept = |conversation: &Conversation| -> Vec<String> {
        conversation
            .messages()
            .iter()
            .filter_map(|message| match message {
                Message::Image { image, .. } => Some(image.data.clone()),
                _ => None,
            })
            .collect()
    };
    // One picture a turn.
    let turn = |conversation: &mut Conversation, n: u32| {
        conversation.result(&n.to_string(), shot(n));
        conversation.settle();
    };
    for n in 1..=3 {
        turn(&mut conversation, n);
    }
    assert_eq!(kept(&conversation), ["data1", "data2", "data3"]);
    turn(&mut conversation, 4);
    assert_eq!(
        kept(&conversation),
        ["data4"],
        "the older three went together"
    );
    assert_eq!(
        conversation.messages()[2],
        Message::User(IMAGE_OMITTED.into())
    );
    let before = conversation.messages().to_vec();
    turn(&mut conversation, 5);
    turn(&mut conversation, 6);
    assert_eq!(
        &conversation.messages()[..before.len()],
        &before[..],
        "nothing earlier changed"
    );
    assert!(
        matches!(&conversation.messages()[8], Message::Image { caption, .. } if caption.contains("(4x1;"))
    );
}

#[test]
fn a_picture_the_person_attached_follows_their_message_and_is_never_dropped() {
    let mut conversation = Conversation::new("rules");
    let goal = Message::User("Goal: make it look like this".into());
    conversation.push(goal.clone());
    let image = Image {
        media_type: "image/png".into(),
        data: "iVBORw0KGgo=".into(),
        width: 640,
        height: 480,
    };
    conversation.attach("mockup.png", image.clone());
    let attached = |conversation: &Conversation| {
        let messages = conversation.messages();
        let at = messages
            .iter()
            .position(|message| *message == goal)
            .unwrap();
        match &messages[at + 1] {
            Message::Image {
                caption,
                image: kept,
            } => caption.contains("mockup.png") && *kept == image,
            _ => false,
        }
    };
    assert!(attached(&conversation), "it follows the person's message");
    for n in 1..=4 {
        conversation.result(&n.to_string(), shot(n));
        conversation.settle();
    }
    assert!(
        attached(&conversation),
        "past three tool pictures it is still there"
    );
}

#[test]
fn pictures_wait_until_the_turn_is_settled() {
    let mut conversation = Conversation::new("rules");
    conversation.result("1", shot(1));
    conversation.result("2", output("read", 10));
    assert!(
        !conversation
            .messages()
            .iter()
            .any(|message| matches!(message, Message::Image { .. })),
        "nothing comes between one turn's results"
    );
    conversation.settle();
    let messages = conversation.messages();
    assert!(matches!(messages[1], Message::ToolResult { .. }));
    assert!(matches!(messages[2], Message::ToolResult { .. }));
    assert!(matches!(messages[3], Message::Image { .. }));
}

#[test]
fn a_result_without_state_leaves_the_newest_state_alone() {
    let mut conversation = Conversation::new("rules");
    let stateless = |report: &str| ToolOutput {
        report: report.into(),
        ..ToolOutput::default()
    };
    for i in 0..6 {
        conversation.result(&i.to_string(), stateless(&"r".repeat(MAX_RESULT - 100)));
    }
    conversation.result("look", output("opened view", 500));
    conversation.result("read", stateless("4 ch"));
    let contents = results(&conversation);
    assert!(
        contents[6].starts_with("opened view\n\nxxx"),
        "no newer state follows: {}",
        &contents[6][..40]
    );
}

#[test]
fn a_rewind_forgets_the_states_and_pictures_past_it() {
    let mut conversation = Conversation::new("rules");
    // The old state's index is a result again after the rewind, so a stale index would show.
    conversation.push(Message::User("Goal: first".into()));
    conversation.result("old", output("opened old", 10));
    for n in 1..=3 {
        conversation.result(&n.to_string(), shot(n));
    }
    conversation.settle();
    conversation.truncate(0);
    assert_eq!(conversation.messages().len(), 1, "the rules stay");
    conversation.push(Message::User("Goal: again".into()));
    for n in 4..=7 {
        conversation.result(&n.to_string(), shot(n));
        conversation.settle();
    }
    let messages = conversation.messages();
    assert_eq!(messages[1], Message::User("Goal: again".into()));
    let omitted = messages
        .iter()
        .filter(|message| **message == Message::User(IMAGE_OMITTED.into()))
        .count();
    assert_eq!(omitted, 3, "only the new older pictures went");
    assert!(matches!(messages.last(), Some(Message::Image { .. })));
    // Past the budget, only states still in the conversation are dropped.
    for i in 0..6 {
        conversation.result(&format!("s{i}"), output("opened", MAX_RESULT - 100));
    }
    assert!(
        results(&conversation)
            .iter()
            .all(|content| !content.starts_with("opened old")),
        "a state past the rewind is never written back"
    );
}
