use serde_json::Value;

use super::*;

/// The arguments of a frame `F(...)`, read back as the page's script reads them.
fn args(frame: &str) -> Vec<Value> {
    let inner = frame
        .strip_prefix("F(")
        .and_then(|rest| rest.strip_suffix(')'))
        .unwrap_or_else(|| panic!("not a frame: {frame}"));
    serde_json::from_str(&format!("[{inner}]")).unwrap()
}

/// The rows, the cursor and whether it is hidden, after `bytes` (fed whole, then a byte at a time:
/// the screen must not depend on how the reads were cut).
fn shown(bytes: &[u8]) -> (Vec<String>, (u64, u64), bool) {
    let mut whole = Screen::new(40, 120);
    whole.feed(bytes);
    let mut split = Screen::new(40, 120);
    for byte in bytes {
        split.feed(&[*byte]);
    }
    assert_eq!(
        whole.frame(None),
        split.frame(None),
        "split one byte per read"
    );
    let args = args(&whole.frame(None));
    let rows = args[0].as_array().unwrap();
    let rows = rows.iter().map(|row| row.as_str().unwrap().to_string());
    let cursor = (args[1].as_u64().unwrap(), args[2].as_u64().unwrap());
    (rows.collect(), cursor, args[3].as_bool().unwrap())
}

#[test]
fn lines_land_on_rows_and_the_cursor_follows() {
    let (rows, cursor, hidden) = shown(b"hi\r\nthere");
    assert_eq!(rows.len(), 40);
    assert_eq!(rows[0], "hi");
    assert_eq!(rows[1], "there");
    assert!(rows[2..].iter().all(String::is_empty));
    assert_eq!(cursor, (1, 5));
    assert!(!hidden);
}

#[test]
fn a_carriage_return_redraws_the_row() {
    assert_eq!(shown(b"50%\r100%").0[0], "100%");
}

#[test]
fn an_erased_screen_is_empty() {
    let (rows, cursor, _) = shown(b"one\r\ntwo\x1b[2J\x1b[H");
    assert!(rows.iter().all(String::is_empty), "{rows:?}");
    assert_eq!(cursor, (0, 0));
}

#[test]
fn a_hidden_cursor_is_hidden() {
    assert!(shown(b"x\x1b[?25l").2);
    assert!(!shown(b"x\x1b[?25l\x1b[?25h").2);
}

#[test]
fn untrusted_text_comes_back_as_the_same_text() {
    let text = "<script>alert(1)</script> \"q\" back\\slash \u{2028}line\u{2029} </div>";
    let mut bytes = text.as_bytes().to_vec();
    bytes.extend_from_slice(b"\r\nbad \xff\xfe utf-8");
    let (rows, ..) = shown(&bytes);
    assert_eq!(rows[0], text);
    // Bytes that are not UTF-8 are dropped.
    assert_eq!(rows[1], "bad  utf-8");
    let mut frame = Screen::new(40, 120);
    frame.feed(text.as_bytes());
    let frame = frame.frame(None);
    for raw in ['<', '\u{2028}', '\u{2029}'] {
        assert!(!frame.contains(raw), "{raw:?} is escaped in {frame}");
    }
}

#[test]
fn only_text_reaches_the_screen() {
    // A title, a clipboard write, a hyperlink, a device control string, a query, a bell, a C1
    // control and NUL: none of them shows, and the text around them does.
    let bytes = "a\x1b]0;title\x07b\x1b]52;c;aGk=\x1b\\c\x1b]8;;http://x\x07d\x1bP1$r0m\x1b\\e\
                 \x1b[6nf\x07g\u{9b}31mh\x00i"
        .as_bytes();
    let (rows, ..) = shown(bytes);
    assert_eq!(rows[0], "abcdefg31mhi");
    // Tabs and backspaces move the cursor as a terminal's do.
    let (rows, cursor, _) = shown(b"ab\x08c\td");
    assert_eq!(rows[0], "ac      d");
    assert_eq!(cursor, (0, 9));
}

#[test]
fn an_ended_session_says_so_in_the_title() {
    let screen = Screen::new(40, 120);
    assert_eq!(args(&screen.frame(None))[4], Value::Null);
    let ended = args(&screen.frame(Some(" · exited 0")));
    assert_eq!(ended[4], " · exited 0");
}

#[test]
fn the_page_takes_the_persons_colours() {
    let theme = Theme {
        fg: [0x10, 0x20, 0x30],
        bg: [0xfe, 0xdc, 0xba],
        cursor: [0, 0, 0xff],
    };
    let themed = page("session 1 · <b>&", Some(&theme), None);
    assert!(themed.contains("--fg:#102030"), "{themed}");
    assert!(themed.contains("--bg:#fedcba"));
    assert!(themed.contains("--cursor:#0000ff"));
    assert!(themed.contains("<title>session 1 · &lt;b&gt;&amp;</title>"));
    assert!(themed.contains("Content-Security-Policy"));
    // Without them, the system's light or dark.
    let plain = page("session 1", None, None);
    assert!(plain.contains("prefers-color-scheme: dark"));
    assert!(!plain.contains("--fg:#102030"));
}

#[test]
fn a_frame_carries_the_screen_as_a_terminal_draws_it() {
    let mut screen = Screen::new(40, 120);
    screen.feed(b"hi\r\n\x1b[1mthere\x1b[0m and \x1b[31mred\x1b[m\x1b[3;7H");
    let drawn = args(&screen.frame(None))[5].as_str().unwrap().to_string();
    // What a terminal shows after it: the same text, styles and cursor.
    let mut again = vt100::Parser::new(40, 120, 0);
    again.process(drawn.as_bytes());
    let (before, after) = (screen.parser.screen(), again.screen());
    assert_eq!(after.contents(), before.contents());
    assert_eq!(after.cursor_position(), (2, 6));
    assert!(after.cell(1, 0).unwrap().bold());
    assert_eq!(after.cell(1, 10).unwrap().fgcolor(), vt100::Color::Idx(1));
    // It draws, and asks nothing: no string sequence, no query, no title.
    assert!(!drawn.contains("\x1b]") && !drawn.contains("\x1bP") && !drawn.contains("[6n"));
}

fn emulator(script: &str) -> Emulator {
    Emulator {
        script: Asset {
            text: script.into(),
            sha384: "AAAA".into(),
        },
        fit: Asset {
            text: "var FitAddon = {};".into(),
            sha384: "BBBB".into(),
        },
        css: ".xterm { cursor: text }".into(),
    }
}

#[test]
fn the_terminal_page_runs_the_verified_scripts_and_nothing_else() {
    let page = page(
        "session 2 · <i>",
        None,
        Some(&emulator("var Terminal = 1;")),
    );
    assert!(
        page.contains("<script>var Terminal = 1;</script>"),
        "{page}"
    );
    assert!(page.contains("<script>var FitAddon = {};</script>"));
    assert!(page.contains(&format!("<script>{GLUE}</script>")));
    assert!(page.contains("<style>.xterm { cursor: text }</style>"));
    let policy = page
        .split("script-src ")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap();
    assert_eq!(
        policy,
        format!("'sha384-AAAA' 'sha384-BBBB' 'sha384-{GLUE_SHA384}'")
    );
    assert!(page.contains("default-src 'none'"));
    assert!(page.contains("<title>session 2 · &lt;i&gt;</title>"));
    assert!(page.contains("--cols:120;--rows:40"));
    assert!(!page.contains("{{"), "{page}");
}

#[test]
fn the_glues_hash_is_its_own() {
    use base64::Engine;
    use sha2::{Digest, Sha384};
    let hash = base64::engine::general_purpose::STANDARD.encode(Sha384::digest(GLUE));
    assert_eq!(GLUE_SHA384, hash);
}

#[test]
fn a_script_that_would_end_its_own_element_gets_the_plain_grid() {
    let plain = page("s", None, None);
    for script in ["a</script>b", "a</SCRIPT >b"] {
        assert_eq!(page("s", None, Some(&emulator(script))), plain);
    }
}
