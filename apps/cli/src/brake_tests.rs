use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use contracts::chat::Message;

use super::*;

/// A chat model that answers every request with the same text, and counts them.
#[derive(Default)]
struct Ready(AtomicUsize);

/// A chat model whose every answer costs 0.01.
struct Dear;

impl Chat for Dear {
    fn complete(&self, _: Request) -> Result<Completion, ChatError> {
        Ok(Completion {
            usage: contracts::chat::Usage {
                cost: 0.01,
                ..Default::default()
            },
            ..Completion::default()
        })
    }
}

impl Chat for Ready {
    fn complete(&self, _: Request) -> Result<Completion, ChatError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Completion {
            text: Some("ready".into()),
            ..Completion::default()
        })
    }
}

fn request(conversation: &[Message]) -> Request<'_> {
    Request {
        conversation,
        tools: &[],
        may_call: true,
    }
}

#[test]
fn past_its_time_a_task_stops_and_the_next_one_starts_fresh() {
    let chat = Ready::default();
    let open = || false;
    let brake = Brake::new(&chat, &open, &open);
    brake.arm(Limits::time(Duration::ZERO));
    assert!(brake.complete(request(&[])).is_err());
    assert_eq!(brake.halted(), Some(Halt::TimedOut));
    assert_eq!(chat.0.load(Ordering::SeqCst), 0, "the model was not called");
    brake.arm(Limits::time(Duration::from_secs(60)));
    assert_eq!(brake.halted(), None);
    assert!(brake.complete(request(&[])).is_ok());
}

#[test]
fn a_wait_past_the_deadline_is_due_and_halts_the_task() {
    let chat = Ready::default();
    let open = || false;
    let brake = Brake::new(&chat, &open, &open);
    assert!(!brake.due(), "no task, no deadline");
    brake.arm(Limits::time(Duration::from_secs(60)));
    assert!(!brake.due());
    assert_eq!(brake.halted(), None);
    brake.arm(Limits::time(Duration::ZERO));
    assert!(brake.due());
    assert_eq!(brake.halted(), Some(Halt::TimedOut));
}

#[test]
fn time_spent_waiting_on_the_person_moves_the_deadline() {
    let chat = Ready::default();
    let open = || false;
    let brake = Brake::new(&chat, &open, &open);
    brake.arm(Limits::time(Duration::ZERO));
    brake.extend(Duration::from_secs(60));
    assert!(
        brake.complete(request(&[])).is_ok(),
        "the wait was given back"
    );
    assert_eq!(brake.halted(), None);
}

#[test]
fn a_closed_output_stops_the_task() {
    let chat = Ready::default();
    let (closed, open) = (|| true, || false);
    let brake = Brake::new(&chat, &closed, &open);
    brake.arm(Limits::time(Duration::from_secs(60)));
    assert!(brake.complete(request(&[])).is_err());
    assert_eq!(brake.halted(), Some(Halt::Closed));
}

#[test]
fn a_stopped_task_halts_at_its_next_turn_and_says_so_after_its_last() {
    let chat = Ready::default();
    let (open, stopped) = (|| false, AtomicBool::new(false));
    let stop = || stopped.load(Ordering::SeqCst);
    let brake = Brake::new(&chat, &open, &stop);
    brake.arm(Limits::time(Duration::from_secs(60)));
    assert!(brake.complete(request(&[])).is_ok());
    stopped.store(true, Ordering::SeqCst);
    assert_eq!(
        brake.halted(),
        Some(Halt::Interrupted),
        "after the last turn"
    );
    assert!(brake.complete(request(&[])).is_err());
    assert_eq!(
        chat.0.load(Ordering::SeqCst),
        1,
        "the model was not asked again"
    );
    assert_eq!(brake.halted(), Some(Halt::Interrupted));
}

#[test]
fn a_task_over_its_budget_halts_before_the_next_request_and_a_new_one_starts_at_zero() {
    let chat = Dear;
    let open = || false;
    let brake = Brake::new(&chat, &open, &open);
    let limits = Limits {
        time: Duration::from_secs(60),
        budget: Some(0.015),
    };
    brake.arm(limits);
    assert!(brake.complete(request(&[])).is_ok());
    assert!(!brake.due());
    assert!(brake.complete(request(&[])).is_ok(), "0.01 is under 0.015");
    assert!(brake.due(), "0.02 is over it, and a wait ends");
    let refused = brake.complete(request(&[])).unwrap_err();
    assert!(refused.to_string().contains("budget"), "{refused}");
    assert_eq!(brake.halted(), Some(Halt::OverBudget));
    brake.arm(limits);
    assert_eq!(brake.halted(), None);
    assert!(brake.complete(request(&[])).is_ok());
}

#[test]
fn no_budget_never_halts() {
    let chat = Dear;
    let open = || false;
    let brake = Brake::new(&chat, &open, &open);
    brake.arm(Limits::time(Duration::from_secs(60)));
    for _ in 0..50 {
        assert!(brake.complete(request(&[])).is_ok());
    }
    assert!(!brake.due());
}
