use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use contracts::chat::Message;

use super::*;

/// A chat model that answers every request with the same text, and counts them.
#[derive(Default)]
struct Ready(AtomicUsize);

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
    brake.arm(Duration::ZERO);
    assert!(brake.complete(request(&[])).is_err());
    assert_eq!(brake.halted(), Some(Halt::TimedOut));
    assert_eq!(chat.0.load(Ordering::SeqCst), 0, "the model was not called");
    brake.arm(Duration::from_secs(60));
    assert_eq!(brake.halted(), None);
    assert!(brake.complete(request(&[])).is_ok());
}

#[test]
fn a_wait_past_the_deadline_is_due_and_halts_the_task() {
    let chat = Ready::default();
    let open = || false;
    let brake = Brake::new(&chat, &open, &open);
    assert!(!brake.due(), "no task, no deadline");
    brake.arm(Duration::from_secs(60));
    assert!(!brake.due());
    assert_eq!(brake.halted(), None);
    brake.arm(Duration::ZERO);
    assert!(brake.due());
    assert_eq!(brake.halted(), Some(Halt::TimedOut));
}

#[test]
fn time_spent_waiting_on_the_person_moves_the_deadline() {
    let chat = Ready::default();
    let open = || false;
    let brake = Brake::new(&chat, &open, &open);
    brake.arm(Duration::ZERO);
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
    brake.arm(Duration::from_secs(60));
    assert!(brake.complete(request(&[])).is_err());
    assert_eq!(brake.halted(), Some(Halt::Closed));
}

#[test]
fn a_stopped_task_halts_at_its_next_turn_and_says_so_after_its_last() {
    let chat = Ready::default();
    let (open, stopped) = (|| false, AtomicBool::new(false));
    let stop = || stopped.load(Ordering::SeqCst);
    let brake = Brake::new(&chat, &open, &stop);
    brake.arm(Duration::from_secs(60));
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
