use std::cell::Cell;

use contracts::chat::Message;

use super::*;

/// A chat model that answers every request with the same text, and counts them.
#[derive(Default)]
struct Ready(Cell<usize>);

impl Chat for Ready {
    fn complete(&self, _: Request) -> Result<Completion, ChatError> {
        self.0.set(self.0.get() + 1);
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
fn within_its_time_a_task_reaches_the_model() {
    let chat = Ready::default();
    let open = || false;
    let brake = Brake::new(&chat, &open, &open);
    brake.arm(Duration::from_secs(60));
    assert!(brake.complete(request(&[])).is_ok());
    assert_eq!(chat.0.get(), 1);
    assert_eq!(brake.halted(), None);
}

#[test]
fn past_its_time_a_task_stops_and_the_next_one_starts_fresh() {
    let chat = Ready::default();
    let open = || false;
    let brake = Brake::new(&chat, &open, &open);
    brake.arm(Duration::ZERO);
    assert!(brake.complete(request(&[])).is_err());
    assert_eq!(brake.halted(), Some(Halt::TimedOut));
    assert_eq!(chat.0.get(), 0, "the model was not called");
    brake.arm(Duration::from_secs(60));
    assert_eq!(brake.halted(), None);
    assert!(brake.complete(request(&[])).is_ok());
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
    let (open, stopped) = (|| false, Cell::new(false));
    let stop = || stopped.get();
    let brake = Brake::new(&chat, &open, &stop);
    brake.arm(Duration::from_secs(60));
    assert!(brake.complete(request(&[])).is_ok());
    stopped.set(true);
    assert_eq!(
        brake.halted(),
        Some(Halt::Interrupted),
        "after the last turn"
    );
    assert!(brake.complete(request(&[])).is_err());
    assert_eq!(chat.0.get(), 1, "the model was not asked again");
    assert_eq!(brake.halted(), Some(Halt::Interrupted));
}

#[test]
fn a_stopped_task_asks_the_decision_model_nothing_more() {
    struct Never;
    impl Decider for Never {
        fn decide(
            &self,
            _: &Value,
            _: &BTreeMap<String, Question>,
        ) -> Result<Decision, DecideError> {
            panic!("asked after the stop")
        }
    }
    let interrupt = Interrupt::new();
    interrupt.begin();
    interrupt.stop();
    let decider = BrakedDecider {
        inner: &Never,
        interrupt: &interrupt,
    };
    let refused = decider.decide(&Value::Null, &BTreeMap::new());
    assert!(matches!(refused, Err(DecideError::Unavailable(_))));
}
