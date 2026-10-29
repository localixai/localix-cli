use super::*;

#[test]
fn output_up_to_the_limit_is_kept_whole() {
    let full = "a".repeat(HEAD + TAIL);
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
fn long_output_keeps_a_short_start_and_a_long_end() {
    let mut output = Output::default();
    output.push("h".repeat(HEAD).as_bytes());
    for _ in 0..100 {
        output.push("m".repeat(1000).as_bytes());
    }
    output.push("t".repeat(TAIL).as_bytes());
    assert_eq!(
        output.text(),
        format!(
            "{}\n... 100000 characters cut; redirect the output to a file and read it in parts \
             ...\n{}",
            "h".repeat(HEAD),
            "t".repeat(TAIL)
        )
    );
}

#[test]
fn a_cut_falls_between_lines() {
    let mut output = Output::default();
    for n in 0..20_000 {
        output.push(format!("line {n:05}\n").as_bytes());
    }
    let text = output.text();
    let (head, rest) = text.split_once("\n... ").unwrap();
    let (_, tail) = rest.split_once(" ...\n").unwrap();
    assert!(head.lines().all(|l| l.len() == 10), "a head line is cut");
    assert!(tail.lines().all(|l| l.len() == 10), "a tail line is cut");
    assert!(tail.ends_with("line 19999"));
    assert!(tail.len() > 3 * head.len());
}

#[test]
fn a_cut_never_splits_a_character() {
    let mut output = Output::default();
    // Two-byte characters after one byte, with no line to cut at: the head ends half way through
    // one.
    output.push(b"x");
    for _ in 0..HEAD + TAIL {
        output.push("\u{e9}".as_bytes());
    }
    let text = output.text();
    assert!(
        !text.contains('\u{fffd}'),
        "a replacement character in the cut text"
    );
    let (head, rest) = text.split_once("\n... ").unwrap();
    let (marker, tail) = rest.split_once(" ...\n").unwrap();
    assert!(head.starts_with('x'));
    assert_eq!(head.chars().count(), 1 + (HEAD - 1) / 2);
    let cut: usize = marker.split_once(' ').unwrap().0.parse().unwrap();
    let kept = head.chars().count() + tail.chars().count();
    assert_eq!(kept + cut, 1 + HEAD + TAIL);
}

#[test]
fn lines_are_counted_whole_or_cut() {
    let mut output = Output::default();
    output.push(b"a\nb");
    assert_eq!((output.lines(), output.cut()), (2, false));
    output.push(b"\n");
    assert_eq!(output.lines(), 2);
    for _ in 0..20_000 {
        output.push(b"more\n");
    }
    assert_eq!((output.lines(), output.cut()), (20_002, true));
}
