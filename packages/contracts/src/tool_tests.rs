use super::*;

#[test]
fn a_report_alone_is_an_output_with_no_state_or_picture() {
    let output = ToolOutput::from("done".to_string());
    assert_eq!(output.report, "done");
    assert_eq!(
        output,
        ToolOutput {
            report: "done".into(),
            ..ToolOutput::default()
        }
    );
}
