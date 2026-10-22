use super::*;

#[test]
fn the_page_carries_its_mark_and_escapes_details() {
    let ok = success("Test app");
    assert!(ok.contains("<svg") && ok.contains("Authentication successful"));
    let failed = failure("OpenRouter sign-in failed.", Some("<script>x</script>"));
    assert!(failed.contains("&lt;script&gt;") && !failed.contains("<script>x"));
}
