use super::*;

#[test]
fn tasks_are_kept_in_order_and_forgetting_one_drops_it_and_those_after() {
    let checkpoints = Checkpoints::default();
    checkpoints.begin("first", 3);
    checkpoints.begin("second", 7);
    checkpoints.begin("third", 9);
    let goals = |c: &Checkpoints| {
        c.tasks()
            .into_iter()
            .map(|t| (t.goal, t.at))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        goals(&checkpoints),
        [
            ("first".into(), 3),
            ("second".into(), 7),
            ("third".into(), 9)
        ]
    );
    checkpoints.forget(1);
    assert_eq!(goals(&checkpoints), [("first".to_string(), 3)]);
}
