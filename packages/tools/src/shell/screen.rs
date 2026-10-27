//! A terminal session's screen, for a window that shows it: the output applied as a terminal
//! would (cursor moves, erases, redraws), then each row's text. The bytes are untrusted, so only
//! text, line moves (`\n`, `\r`, `\t`, `\b`) and CSI sequences reach the emulator: no title, no
//! clipboard write, no string sequence at all; and only characters that print reach a frame.
//! The page sets each row as text, never as markup.

use super::pty::{COLUMNS, ROWS};

/// A colour as red, green and blue.
pub type Rgb = [u8; 3];

/// The person's terminal colours, which the window takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub fg: Rgb,
    pub bg: Rgb,
    pub cursor: Rgb,
}

pub(super) struct Screen {
    safe: Safe,
    parser: vt100::Parser,
}

impl Screen {
    pub(super) fn new(rows: u16, columns: u16) -> Self {
        Self {
            safe: Safe::default(),
            parser: vt100::Parser::new(rows, columns, 0),
        }
    }

    pub(super) fn feed(&mut self, bytes: &[u8]) {
        let kept = self.safe.keep(bytes);
        self.parser.process(&kept);
    }

    /// Whether the program drew on the alternate screen (a full-screen program).
    pub(super) fn alternate(&self) -> bool {
        self.parser.screen().alternate_screen()
    }

    /// The screen as text, its blank rows at the end left out, and where the cursor is (row and
    /// column, from 1).
    pub(super) fn text(&self) -> (String, (u16, u16)) {
        let screen = self.parser.screen();
        let rows: Vec<String> = screen
            .rows(0, screen.size().1)
            .map(|row| row.chars().filter(|c| !c.is_control()).collect::<String>())
            .map(|row| row.trim_end().to_string())
            .collect();
        let used = rows
            .iter()
            .rposition(|row| !row.is_empty())
            .map_or(0, |at| at + 1);
        (rows[..used].join("\n"), self.cursor())
    }

    /// Where the cursor is: row and column, from 1.
    pub(super) fn cursor(&self) -> (u16, u16) {
        let (row, column) = self.parser.screen().cursor_position();
        (row + 1, column + 1)
    }

    /// The page's call that draws the screen: `F(rows, row, column, hidden, end, drawn)`, each
    /// a JSON value; `end` is what the title gains once the session ended, `drawn` the screen as
    /// a terminal would be told to draw it (for an emulator's page; the grid's reads the rows).
    pub(super) fn frame(&self, end: Option<&str>) -> String {
        let screen = self.parser.screen();
        let columns = screen.size().1;
        let rows: Vec<String> = screen
            .rows(0, columns)
            .map(|row| row.chars().filter(|c| !c.is_control()).collect())
            .collect();
        let (row, column) = screen.cursor_position();
        let hidden = screen.hide_cursor();
        let drawn = String::from_utf8_lossy(&screen.contents_formatted()).into_owned();
        format!(
            "F({},{row},{column},{hidden},{},{})",
            json(&rows),
            json(&end),
            json(&drawn)
        )
    }
}

/// A script or stylesheet the caller fetched and checked, with its SHA-384 (base64).
pub struct Asset {
    pub text: String,
    pub sha384: String,
}

/// A terminal emulator for the page: its script, its fit addon's, and its stylesheet.
pub struct Emulator {
    pub script: Asset,
    pub fit: Asset,
    pub css: String,
}

/// The emulator page's own script, and its SHA-384: the page's policy runs it by that hash.
pub(super) const GLUE: &str = include_str!("emulator.js");
pub(super) const GLUE_SHA384: &str =
    "AYHYwjMcpsisoH0rfpoht8s8cVPiy8HKg6v6ez23upJARWDTfsDTg2gnOUg7GZga";

/// The page that shows a screen, titled `title`, in the person's colours, else the system's:
/// drawn by `emulator` when there is one, else as a grid of rows. Its policy runs its own scripts
/// only: the emulator's by their hashes.
pub fn page(title: &str, theme: Option<&Theme>, emulator: Option<&Emulator>) -> String {
    let colours = theme.map_or(String::new(), |theme| {
        format!(
            ":root{{--fg:{};--bg:{};--cursor:{}}}",
            hex(theme.fg),
            hex(theme.bg),
            hex(theme.cursor)
        )
    });
    let title = title
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let (columns, rows) = (COLUMNS.to_string(), ROWS.to_string());
    // Text that would end its element early would let the rest run as markup.
    let inert = |text: &str| {
        let text = text.to_ascii_lowercase();
        !text.contains("</script") && !text.contains("</style")
    };
    let Some(emulator) =
        emulator.filter(|e| inert(&e.script.text) && inert(&e.fit.text) && inert(&e.css))
    else {
        return fill(
            include_str!("screen.html"),
            &[
                ("title", &title),
                ("theme", &colours),
                ("cols", &columns),
                ("rows", &rows),
            ],
        );
    };
    let scripts = format!(
        "'sha384-{}' 'sha384-{}' 'sha384-{GLUE_SHA384}'",
        emulator.script.sha384, emulator.fit.sha384
    );
    fill(
        include_str!("emulator.html"),
        &[
            ("scripts", &scripts),
            ("title", &title),
            ("css", &emulator.css),
            ("cols", &columns),
            ("rows", &rows),
            ("theme", &colours),
            ("xterm", &emulator.script.text),
            ("fit", &emulator.fit.text),
            ("glue", GLUE),
        ],
    )
}

/// `template` with each `{{key}}` in it replaced by its value; what a value holds is never read
/// as a key.
fn fill(template: &str, values: &[(&str, &str)]) -> String {
    let mut page =
        String::with_capacity(template.len() + values.iter().map(|v| v.1.len()).sum::<usize>());
    let mut rest = template;
    while let Some((before, after)) = rest.split_once("{{") {
        page.push_str(before);
        let (key, next) = after.split_once("}}").unwrap_or((after, ""));
        match values.iter().find(|(known, _)| *known == key) {
            Some((_, value)) => page.push_str(value),
            None => {
                page.push_str("{{");
                page.push_str(key);
            }
        }
        rest = next;
    }
    page.push_str(rest);
    page
}

fn hex([red, green, blue]: Rgb) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

/// JSON that is also safe as script text anywhere: no `<`, and no line separators that older
/// engines end a string at.
fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .unwrap_or_default()
        .replace('<', "\\u003c")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum State {
    #[default]
    Ground,
    Escape,
    Csi,
    /// An OSC, DCS, APC, PM or SOS string, dropped up to BEL or `ESC \`.
    Text,
    TextEscape,
    /// After `ESC (` and its kin: one more byte, dropped.
    Charset,
}

/// What of the output may reach the emulator; its state carries over between reads.
#[derive(Debug, Default)]
struct Safe {
    state: State,
}

impl Safe {
    fn keep(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut kept = Vec::with_capacity(bytes.len());
        for &byte in bytes {
            self.state = match (self.state, byte) {
                (State::Ground, 0x1b) => State::Escape,
                (State::Ground, b'\n' | b'\r' | b'\t' | 0x08) => {
                    kept.push(byte);
                    State::Ground
                }
                (State::Ground, ..0x20 | 0x7f) => State::Ground,
                (State::Ground, _) => {
                    kept.push(byte);
                    State::Ground
                }
                (State::Escape, b'[') => {
                    kept.extend_from_slice(b"\x1b[");
                    State::Csi
                }
                (State::Escape, b']' | b'P' | b'X' | b'^' | b'_') => State::Text,
                (State::Escape, 0x20..=0x2f) => State::Charset,
                (State::Escape, 0x1b) => State::Escape,
                // Two-byte escapes (save and restore the cursor, a reverse index) are kept;
                // anything else is dropped.
                (State::Escape, b'7' | b'8' | b'M' | b'D' | b'E') => {
                    kept.extend_from_slice(&[0x1b, byte]);
                    State::Ground
                }
                (State::Escape, _) | (State::Charset, _) => State::Ground,
                (State::Csi, 0x20..=0x3f) => {
                    kept.push(byte);
                    State::Csi
                }
                (State::Csi, 0x40..=0x7e) => {
                    kept.push(byte);
                    State::Ground
                }
                // A sequence broken off: what was kept of it is ended harmlessly, as `m`.
                (State::Csi, 0x1b) => {
                    kept.push(b'm');
                    State::Escape
                }
                (State::Csi, _) => {
                    kept.push(b'm');
                    State::Ground
                }
                (State::Text, 0x07) => State::Ground,
                (State::Text, 0x1b) => State::TextEscape,
                (State::Text, _) => State::Text,
                (State::TextEscape, b'\\') => State::Ground,
                (State::TextEscape, _) => State::Text,
            };
        }
        kept
    }
}

#[cfg(test)]
#[path = "screen_tests.rs"]
mod tests;
