use super::*;

#[test]
fn output_up_to_the_limit_is_kept_whole() {
    let full = "a".repeat(2 * HALF);
    // (the pushes, the text): trailing line breaks are dropped.
    let cases: [(Vec<&[u8]>, &str); 2] = [
        (vec![b"hello ", b"world\n\n"], "hello world"),
        (vec![full.as_bytes()], &full),
    ];
    for (pushes, expected) in cases {
        let mut output = Output::default();
        for push in pushes {
            output.push(push);
        }
        assert_eq!(output.text(), expected);
    }
}

#[test]
fn long_output_keeps_its_start_and_end() {
    let mut output = Output::default();
    output.push("h".repeat(HALF).as_bytes());
    for _ in 0..100 {
        output.push("m".repeat(1000).as_bytes());
    }
    output.push("t".repeat(HALF).as_bytes());
    assert_eq!(
        output.text(),
        format!(
            "{}\n... 100000 characters cut ...\n{}",
            "h".repeat(HALF),
            "t".repeat(HALF)
        )
    );
}

#[test]
fn a_cut_never_splits_a_character() {
    let mut output = Output::default();
    // Two-byte characters after one byte: the head ends half way through one.
    output.push(b"x");
    for _ in 0..HALF {
        output.push("\u{e9}".as_bytes());
    }
    let text = output.text();
    assert!(
        !text.contains('\u{fffd}'),
        "a replacement character in the cut text"
    );
    let (head, rest) = text.split_once("\n... ").unwrap();
    let (marker, tail) = rest.split_once(" characters cut ...\n").unwrap();
    assert!(head.starts_with('x'));
    assert_eq!(head.chars().count(), 1 + (HALF - 1) / 2);
    let kept = head.chars().count() + tail.chars().count();
    assert_eq!(kept + marker.parse::<usize>().unwrap(), 1 + HALF);
}
