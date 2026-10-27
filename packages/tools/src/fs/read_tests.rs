use serde_json::json;

use super::*;
use crate::testing::Scratch;

#[expect(
    clippy::unwrap_in_result,
    clippy::panic_in_result_fn,
    reason = "a test's helper: a setup that fails, fails the test"
)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "a test helper takes its arguments as the tests write them, by value"
)]
fn read(dir: &Scratch, arguments: Value) -> Result<String, ToolError> {
    let workspace = Workspace::new(dir.path()).unwrap();
    let tool = ReadTool {
        workspace: &workspace,
    };
    tool.call(&arguments).map(|output| {
        assert_eq!(output.state, None);
        output.report
    })
}

fn failed(result: Result<String, ToolError>) -> String {
    match result {
        Err(ToolError::Failed(message)) => message,
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn a_file_comes_back_numbered_with_a_header() {
    let dir = Scratch::new();
    dir.file("src/a.txt", "one\ntwo\r\n\nfour");
    assert_eq!(
        read(&dir, json!({"path": "src/a.txt"})).unwrap(),
        "src/a.txt (lines 1-4 of 4)\n     1\tone\n     2\ttwo\n     3\t\n     4\tfour"
    );
}

#[expect(clippy::format_collect, reason = "a test's fixture, built once")]
#[test]
fn offset_and_limit_pick_a_window_and_say_how_to_go_on() {
    let dir = Scratch::new();
    let text: String = (1..=10).map(|n| format!("line {n}\n")).collect();
    dir.file("ten.txt", text);
    assert_eq!(
        read(&dir, json!({"path": "ten.txt", "offset": 4, "limit": 2})).unwrap(),
        "ten.txt (lines 4-5 of 10)\n     4\tline 4\n     5\tline 5\n\n\
         (lines 6-10 not shown: read again with offset 6 to continue)"
    );
    assert_eq!(
        read(&dir, json!({"path": "ten.txt", "offset": 10})).unwrap(),
        "ten.txt (lines 10-10 of 10)\n    10\tline 10"
    );
    assert_eq!(
        failed(read(&dir, json!({"path": "ten.txt", "offset": 11}))),
        "ten.txt has 10 lines: offset 11 is past the end"
    );
}

#[test]
fn the_default_window_is_two_thousand_lines() {
    let dir = Scratch::new();
    // Empty lines: 2000 of them stay within the byte budget.
    dir.file("long.txt", "\n".repeat(2500));
    let report = read(&dir, json!({"path": "long.txt"})).unwrap();
    assert!(report.starts_with("long.txt (lines 1-2000 of 2500)\n"));
    assert!(report.ends_with("read again with offset 2001 to continue)"));
}

#[expect(clippy::format_collect, reason = "a test's fixture, built once")]
#[test]
fn a_window_stays_within_its_byte_budget() {
    let dir = Scratch::new();
    let text: String = (0..100)
        .map(|_| format!("{}\n", "y".repeat(1000)))
        .collect();
    dir.file("wide.txt", text);
    let report = read(&dir, json!({"path": "wide.txt"})).unwrap();
    assert!(report.len() < MAX_BYTES + 200, "{} bytes", report.len());
    assert!(report.starts_with("wide.txt (lines 1-19 of 100)\n"));
    assert!(report.ends_with("read again with offset 20 to continue)"));
}

#[test]
fn an_overlong_line_is_cut_with_its_length() {
    let dir = Scratch::new();
    dir.file("min.js", format!("{}\nnext\n", "z".repeat(100_000)));
    let report = read(&dir, json!({"path": "min.js"})).unwrap();
    let first = report.lines().nth(1).unwrap();
    assert_eq!(
        first,
        format!(
            "     1\t{} ... [line cut: 100000 bytes]",
            "z".repeat(LINE_CHARS)
        )
    );
    assert!(report.ends_with("     2\tnext"));
}

#[test]
fn an_empty_file_says_so() {
    let dir = Scratch::new();
    dir.file("empty.txt", "");
    assert_eq!(
        read(&dir, json!({"path": "empty.txt"})).unwrap(),
        "empty.txt (empty file)"
    );
}

#[test]
fn a_binary_file_is_refused() {
    let dir = Scratch::new();
    dir.file("binary.dat", b"\0binary");
    assert_eq!(
        failed(read(&dir, json!({"path": "binary.dat"}))),
        "binary.dat is a binary file: it cannot be read as text"
    );
}

#[test]
fn image_formats_are_returned_by_signature_with_dimensions() {
    use base64::Engine as _;
    let dir = Scratch::new();
    let fixtures: &[(&[u8], &str)] = &[
        (
            b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x03\0\0\0\x02",
            "image/png",
        ),
        (b"\xff\xd8\xff\xc0\0\x07\x08\0\x02\0\x03", "image/jpeg"),
        (b"GIF89a\x03\0\x02\0", "image/gif"),
        (
            b"RIFF\x16\0\0\0WEBPVP8X\x0a\0\0\0\0\0\0\0\x02\0\0\x01\0\0",
            "image/webp",
        ),
        (
            b"RIFF\x11\0\0\0WEBPVP8L\x05\0\0\0\x2f\x02\x40\0\0",
            "image/webp",
        ),
        (
            b"RIFF\x16\0\0\0WEBPVP8 \x0a\0\0\0\0\0\0\x9d\x01\x2a\x03\0\x02\0",
            "image/webp",
        ),
    ];
    let workspace = Workspace::new(dir.path()).unwrap();
    let tool = ReadTool {
        workspace: &workspace,
    };
    for (bytes, media) in fixtures {
        dir.file("file.dat", bytes);
        let output = tool.call(&json!({"path": "file.dat"})).unwrap();
        let image = output.image.unwrap();
        assert_eq!(image.media_type, *media);
        assert_eq!((image.width, image.height), (3, 2));
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(image.data)
                .unwrap(),
            *bytes
        );
    }
}

#[test]
fn a_real_png_is_returned_intact() {
    use base64::Engine as _;
    // A complete 1 × 1 PNG, including IDAT and IEND chunks.
    let encoded = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .unwrap();
    let dir = Scratch::new();
    dir.file("pixel.png", &bytes);
    let workspace = Workspace::new(dir.path()).unwrap();
    let output = ReadTool {
        workspace: &workspace,
    }
    .call(&json!({"path": "pixel.png"}))
    .unwrap();
    assert_eq!(output.report, "pixel.png (image/png, 1 × 1)");
    assert_eq!(output.state, None);
    let image = output.image.unwrap();
    assert_eq!(image.media_type, "image/png");
    assert_eq!((image.width, image.height), (1, 1));
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(image.data)
            .unwrap(),
        bytes
    );
}

#[test]
fn denied_image_paths_are_refused() {
    let dir = Scratch::new();
    let denied = dir.file("private/pic.png", b"GIF89a\x01\0\x01\0");
    let workspace = Workspace::new(dir.path()).unwrap().denying([denied]);
    let tool = ReadTool {
        workspace: &workspace,
    };
    let error = tool.call(&json!({"path": "private/pic.png"})).unwrap_err();
    assert!(matches!(error, ToolError::Failed(message) if message.contains("off limits")));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            dir.path().join("private/pic.png"),
            dir.path().join("link.png"),
        )
        .unwrap();
        let error = tool.call(&json!({"path": "link.png"})).unwrap_err();
        assert!(matches!(error, ToolError::Failed(message) if message.contains("off limits")));
    }
}

#[test]
fn image_size_limit_includes_the_boundary_and_catches_a_growing_reader() {
    let limit = 5 * 1024 * 1024;
    let mut bytes = vec![0; limit];
    bytes[..10].copy_from_slice(b"GIF89a\x01\0\x01\0");
    let dir = Scratch::new();
    dir.file("limit.gif", &bytes);
    let workspace = Workspace::new(dir.path()).unwrap();
    let output = ReadTool {
        workspace: &workspace,
    }
    .call(&json!({"path": "limit.gif"}))
    .unwrap();
    assert_eq!(
        (output.image.unwrap().width, output.report.as_str()),
        (1, "limit.gif (image/gif, 1 × 1)")
    );

    // The file may grow after metadata is read: bound the actual stream as well.
    bytes.push(0);
    let args = Args {
        path: "growing.gif".into(),
        offset: None,
        limit: None,
    };
    let mut reader = io::Cursor::new(bytes);
    let error = image::read(
        &mut reader,
        "growing.gif",
        limit as u64,
        image::Format::Gif,
        &args,
    )
    .unwrap_err();
    assert!(matches!(error, ToolError::Failed(message) if message.contains("5 MiB")));
    assert_eq!(reader.position(), limit as u64 + 1);
}

#[test]
fn images_reject_line_arguments_bad_headers_and_excess_size() {
    let dir = Scratch::new();
    dir.file("pic", b"GIF89a\x03\0\x02\0");
    for args in [
        json!({"path":"pic", "offset": 1}),
        json!({"path":"pic", "limit": 1}),
    ] {
        assert!(failed(read(&dir, args)).contains("offset and limit"));
    }
    dir.file("pic", b"GIF89a");
    assert!(failed(read(&dir, json!({"path":"pic"}))).contains("invalid or incomplete"));
    let mut large = vec![0; 5 * 1024 * 1024 + 1];
    large[..6].copy_from_slice(b"GIF89a");
    dir.file("pic", large);
    assert!(failed(read(&dir, json!({"path":"pic"}))).contains("5 MiB"));
}

#[test]
fn a_missing_file_is_named() {
    let dir = Scratch::new();
    assert_eq!(
        failed(read(&dir, json!({"path": "nope/missing.rs"}))),
        format!("{} does not exist", Path::new("nope/missing.rs").display())
    );
}

#[test]
fn a_directory_lists_its_entries_sorted() {
    let dir = Scratch::new();
    dir.file("b.txt", "");
    dir.file("a/inner.txt", "");
    dir.file("C.md", "");
    assert_eq!(
        read(&dir, json!({"path": "."})).unwrap(),
        "./ (3 entries)\nC.md\na/\nb.txt"
    );
    assert_eq!(
        read(&dir, json!({"path": "a/"})).unwrap(),
        "a/ (1 entry)\ninner.txt"
    );
}

#[test]
fn a_large_directory_is_capped() {
    let dir = Scratch::new();
    for n in 0..MAX_ENTRIES + 3 {
        dir.file(&format!("f{n:04}"), "");
    }
    let report = read(&dir, json!({"path": "."})).unwrap();
    assert_eq!(report.lines().count(), MAX_ENTRIES + 2);
    assert!(
        report
            .ends_with("... 3 more not listed: use bash (`ls`, `find`, `rg --files`) to search it")
    );
}

#[test]
fn unknown_arguments_are_refused() {
    let dir = Scratch::new();
    assert!(failed(read(&dir, json!({"path": "a", "lines": 3}))).starts_with("unusable arguments"));
}

#[test]
fn next_line_keeps_only_the_start_of_a_line() {
    let mut reader = io::Cursor::new(b"abcdef\n\nxy".to_vec());
    let mut line = vec![];
    assert_eq!(next_line(&mut reader, &mut line, 3).unwrap(), Some(6));
    assert_eq!(line, b"abc");
    assert_eq!(next_line(&mut reader, &mut line, 3).unwrap(), Some(0));
    assert_eq!(next_line(&mut reader, &mut line, 3).unwrap(), Some(2));
    assert_eq!(line, b"xy");
    assert_eq!(next_line(&mut reader, &mut line, 3).unwrap(), None);
}

#[expect(
    clippy::disallowed_methods,
    reason = "a test's fixture: the command ends by itself"
)]
#[cfg(unix)]
#[test]
fn what_is_not_a_regular_file_is_refused_before_it_is_opened() {
    let dir = Scratch::new();
    // Opening a FIFO with no writer would block for ever.
    let made = std::process::Command::new("mkfifo")
        .arg(dir.path().join("pipe"))
        .status()
        .unwrap();
    assert!(made.success());
    assert_eq!(
        failed(read(&dir, json!({"path": "pipe"}))),
        "pipe is not a regular file: it cannot be read here"
    );
    // A device that never ends a line, nor the file.
    assert_eq!(
        failed(read(&dir, json!({"path": "/dev/zero"}))),
        "/dev/zero is not a regular file: it cannot be read here"
    );
}
