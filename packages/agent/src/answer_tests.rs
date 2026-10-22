use super::*;

#[test]
fn the_closing_line_says_whether_the_goal_was_reached() {
    assert_eq!(
        read("Price is 112 CHF.\nRESULT: achieved"),
        ("Price is 112 CHF.".into(), Some(true))
    );
    assert_eq!(
        read("No flights.\n\nRESULT: not achieved"),
        ("No flights.".into(), Some(false))
    );
    assert_eq!(read("Just text"), ("Just text".into(), None));
}
