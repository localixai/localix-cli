use super::*;
use crate::testing::{NoDecider, ScriptedChat, answers};

fn ask(chat: &dyn Chat) -> Result<Completion, ChatError> {
    chat.complete(Request {
        conversation: &[],
        tools: &[],
        may_call: false,
    })
}

#[test]
fn a_session_signed_out_says_how_to_sign_in_until_a_model_is_put_in() {
    let live = LiveChat::new(Box::new(SignedOut(Provider::OpenAi)));
    let refused = ask(&live).unwrap_err().to_string();
    assert!(
        refused.contains("not signed in") && refused.contains("/login"),
        "{refused}"
    );
    live.set(Box::new(ScriptedChat::new(answers(1))));
    assert!(ask(&live).is_ok());
}

#[test]
fn the_browser_without_a_decision_model_says_where_to_get_one() {
    let live = LiveDecider::new(None);
    let refused = live.decide(&Value::Null, &BTreeMap::new()).unwrap_err();
    assert_eq!(refused, DecideError::Unavailable(NO_DECIDER.into()));
    live.set(Some(Box::new(NoDecider)));
    let asked = live.decide(&Value::Null, &BTreeMap::new()).unwrap_err();
    assert!(
        asked.to_string().contains("in tests"),
        "the one put in answers"
    );
}
