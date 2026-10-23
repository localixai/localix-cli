//! The terminal as the session draws it. Finished blocks are printed once, above a live area,
//! and scroll up into the terminal's own scrollback, so scrolling, selecting and copying them is
//! the terminal's and they survive the session. The live area (what runs now, the composer, a
//! question) is the only part redrawn: each frame erases it, prints what finished since, and draws
//! it again, inside one synchronized update so the terminal never shows the half-drawn state.
//! Every row is at most the width, so the rows drawn are the rows counted.

use std::io::{self, Write};

use ratatui::text::Line;

use super::style::ansi;
use super::wrap::fit;

pub struct Screen<W: Write> {
    out: W,
    colour: bool,
    /// The live row the cursor was left on.
    cursor: usize,
}

/// Where the composer's cursor goes: a row of the live area and a column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cursor {
    pub row: usize,
    pub column: usize,
}

impl<W: Write> Screen<W> {
    pub fn new(out: W, colour: bool) -> Self {
        Self {
            out,
            colour,
            cursor: 0,
        }
    }

    /// Prints `history` for good, then draws `live` below it, at most `height - 1` rows of it
    /// (its last ones), with the cursor at `cursor` or hidden.
    pub fn frame(
        &mut self,
        history: &[Line<'static>],
        live: &[Line<'static>],
        cursor: Option<Cursor>,
        height: usize,
        width: usize,
    ) -> io::Result<()> {
        let history: Vec<Line> = history.iter().flat_map(|line| fit(line, width)).collect();
        // A row split in two moves the rows under it, the cursor's among them.
        let mut cursor = cursor;
        let mut split = vec![];
        for (at, line) in live.iter().enumerate() {
            let rows = fit(line, width);
            if let Some(c) = cursor.as_mut().filter(|c| c.row > at) {
                c.row += rows.len() - 1;
            }
            split.extend(rows);
        }
        let live = &split[..];
        let mut out = String::from("\x1b[?2026h");
        self.erase_into(&mut out);
        for line in &history {
            out.push_str(&ansi(line, self.colour));
            out.push_str("\r\n");
        }
        let room = height.saturating_sub(1).max(1);
        let skipped = live.len().saturating_sub(room);
        let live = &live[skipped..];
        for (at, line) in live.iter().enumerate() {
            if at > 0 {
                out.push_str("\r\n");
            }
            out.push_str(&ansi(line, self.colour));
        }
        let last = live.len().saturating_sub(1);
        self.cursor = last;
        match cursor.filter(|c| c.row >= skipped && c.row - skipped <= last) {
            Some(Cursor { row, column }) => {
                let row = row - skipped;
                if row < last {
                    out.push_str(&format!("\x1b[{}A", last - row));
                }
                out.push('\r');
                if column > 0 {
                    out.push_str(&format!("\x1b[{column}C"));
                }
                out.push_str("\x1b[?25h");
                self.cursor = row;
            }
            None => out.push_str("\x1b[?25l"),
        }
        out.push_str("\x1b[?2026l");
        self.out.write_all(out.as_bytes())?;
        self.out.flush()
    }

    /// Takes the live area away and leaves the cursor where it started, shown: for leaving the
    /// session, or for handing the terminal to something else for a while.
    pub fn clear(&mut self) -> io::Result<()> {
        let mut out = String::from("\x1b[?2026h");
        self.erase_into(&mut out);
        out.push_str("\x1b[?25h\x1b[?2026l");
        self.cursor = 0;
        self.out.write_all(out.as_bytes())?;
        self.out.flush()
    }

    /// Clears the screen and its scrollback, the cursor at the top: another session's
    /// transcript is drawn from the start.
    pub fn reset(&mut self) -> io::Result<()> {
        self.cursor = 0;
        self.out
            .write_all(b"\x1b[?2026h\x1b[H\x1b[2J\x1b[3J\x1b[?2026l")?;
        self.out.flush()
    }

    fn erase_into(&self, out: &mut String) {
        if self.cursor > 0 {
            out.push_str(&format!("\x1b[{}A", self.cursor));
        }
        out.push_str("\r\x1b[J");
    }

    #[cfg(test)]
    pub fn output(&self) -> &W {
        &self.out
    }
}

#[cfg(test)]
#[path = "screen_tests.rs"]
mod tests;
