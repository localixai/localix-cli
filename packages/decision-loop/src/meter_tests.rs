use super::*;

#[test]
fn spend_adds_up_and_reads_well() {
    let mut spend = Spend::default();
    spend.add("m", 1000, 50, 0.001, 900);
    spend.add("", 2000, 70, 0.002, 1100);
    assert_eq!(
        (spend.calls, spend.input_tokens, spend.output_tokens),
        (2, 3000, 120)
    );
    assert!((spend.cost - 0.003).abs() < 1e-12);
    assert_eq!(
        spend.model, "m",
        "an answer without a model keeps the last one"
    );
    assert_eq!(thousands(1234567), "1,234,567");
    assert_eq!(thousands(12), "12");
}
