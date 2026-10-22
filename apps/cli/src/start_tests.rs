use super::*;

#[test]
fn a_schema_is_inline_json_or_a_file() {
    assert!(read_schema("{\"type\":\"object\"}").is_ok());
    assert!(read_schema("[1]").is_err());
    assert!(read_schema("@/nonexistent/schema.json").is_err());
}

#[test]
fn a_goal_is_never_empty() {
    assert_eq!(read_goal(Some("  find it ")), Ok("find it".to_string()));
    assert!(read_goal(Some("   ")).is_err());
    assert!(read_goal(None).is_err());
}
