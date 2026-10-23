use super::*;

#[test]
fn insertion_and_deletion_use_zero_ranges_at_empty_edges() {
    assert_eq!(unified("", "a\n"), "@@ -0,0 +1,1 @@\n+a\n");
    assert_eq!(unified("a\n", ""), "@@ -1,1 +0,0 @@\n-a\n");
    assert_eq!(unified("", ""), "");
}

#[test]
fn final_newline_changes_are_not_lost() {
    assert_eq!(
        unified("a", "a\n"),
        "@@ -1,1 +1,1 @@\n-a\n\\ No newline at end of file\n+a\n"
    );
}

#[test]
fn separated_changes_keep_equal_context_and_omit_the_gap() {
    let middle = "same\n".repeat(10);
    let before = format!("old\n{middle}old\n");
    let after = format!("new\n{middle}new\n");
    let diff = unified(&before, &after);
    assert_eq!(diff.matches("-old\n").count(), 2);
    assert_eq!(diff.matches("+new\n").count(), 2);
    assert_eq!(diff.matches("@@ -").count(), 2);
    assert!(diff.starts_with("@@ -1,4 +1,4 @@\n"));
    assert!(diff.contains("@@ -9,4 +9,4 @@\n"));
    assert_eq!(diff.matches(" same\n").count(), 6);
    assert!(!diff.contains("-same\n"));
    assert!(!diff.contains("+same\n"));
}

#[test]
fn a_gap_larger_than_the_output_budget_does_not_hide_changes() {
    let middle = "unchanged\n".repeat(300);
    let diff = unified(
        &format!("old\n{middle}old\n"),
        &format!("new\n{middle}new\n"),
    );
    assert_eq!(diff.matches("-old\n").count(), 2);
    assert_eq!(diff.matches("+new\n").count(), 2);
    assert!(diff.contains("@@ -299,4 +299,4 @@\n"));
    assert!(!diff.contains("truncated"));
    assert!(diff.lines().count() < 30);
}

#[test]
fn overlapping_context_merges_and_line_numbers_track_insertions() {
    let diff = unified(
        "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n",
        "a\nextra\nb\nc\nd\ne\nf\ng\nh\ni\nJ\n",
    );
    assert!(diff.contains("@@ -1,4 +1,5 @@\n"), "{diff}");
    assert!(diff.contains("@@ -7,4 +8,4 @@\n"), "{diff}");
    let merged = unified("a\nb\nc\nd\ne\n", "A\nb\nc\nd\nE\n");
    assert_eq!(merged.matches("@@ -").count(), 1);
    assert!(merged.contains(" b\n c\n d\n"));
}

#[test]
fn oversized_matrix_falls_back_without_exceeding_output_budget() {
    let old = "old\n".repeat(1100);
    let new = "new\n".repeat(1100);
    let diff = unified(&old, &new);
    assert!(diff.starts_with("@@ -1,1100 +1,1100 @@\n"));
    assert!(diff.ends_with(TRUNCATED));
    assert!(diff.lines().count() <= MAX_LINES);
    assert!(diff.len() <= MAX_BYTES);
}

#[test]
fn long_lines_are_omitted_with_a_truncation_notice_not_split() {
    let diff = unified("a\n", &"b".repeat(30_000));
    assert!(diff.len() <= MAX_BYTES);
    assert!(diff.ends_with(TRUNCATED));
    assert!(diff.contains("-a\n"));
}

#[test]
fn crlf_is_retained_in_diff_lines() {
    assert_eq!(unified("a\r\n", "b\r\n"), "@@ -1,1 +1,1 @@\n-a\r\n+b\r\n");
}
