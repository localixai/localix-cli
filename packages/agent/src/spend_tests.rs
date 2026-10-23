use super::*;

#[test]
fn another_runs_spend_is_added_and_the_model_kept() {
    let mut mine = Spend {
        model: "m".into(),
        calls: 2,
        cost: 0.5,
        ..Spend::default()
    };
    let theirs = Spend {
        model: "other".into(),
        calls: 3,
        cost: 0.25,
        input_tokens: 10,
        cached_tokens: 4,
        output_tokens: 6,
        millis: 9,
    };
    mine.absorb(&theirs);
    assert_eq!((mine.model.as_str(), mine.calls, mine.cost), ("m", 5, 0.75));
    assert_eq!(
        (
            mine.input_tokens,
            mine.cached_tokens,
            mine.output_tokens,
            mine.millis
        ),
        (10, 4, 6, 9)
    );
    let mut empty = Spend::default();
    empty.absorb(&theirs);
    assert_eq!(empty.model, "other");
}
