//! What a terminal printed, as a call reads it: the transcript since the last call, the line a
//! prompt waits on, and whether the text a call waits for has appeared. The reader thread fills
//! it; a call takes from it. Nothing is lost between calls: what comes while no call waits stays
//! until the next one.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use super::filter::Filter;
use super::output::Output;
use super::pty::{COLUMNS, ROWS};
use super::screen::Screen;

/// A window's painter: a frame in, whether the window is still open out. It is the app's code: the
/// stream calls it, and lets it go, only with its lock let go (ARCHITECTURE.md R6).
pub type Painter = Box<dyn Fn(String) -> bool + Send + Sync>;
pub(super) const PAINT_EVERY: Duration = Duration::from_millis(33);

/// The output read so far, and the wake for who waits on it.
#[derive(Default)]
pub(super) struct Shared {
    state: Mutex<Stream>,
    pub wake: Condvar,
}

impl Shared {
    pub(super) fn lock(&self) -> MutexGuard<'_, Stream> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Tell the window what the stream left for it, with no lock held: a frame to paint (a window
    /// the person closed is let go, unless another took its place meanwhile), or its leaving.
    pub(super) fn tell(&self, paint: Option<Paint>) {
        let Some(Paint { painter, frame }) = paint else {
            return;
        };
        if let Some(frame) = frame
            && !painter(frame)
        {
            self.lock().gone(&painter);
        }
    }
}

/// What the window is told once the stream's lock is let go: a frame, or none when the window is
/// let go (its painter is dropped then, as it closes the window).
#[must_use]
pub(super) struct Paint {
    painter: Arc<Painter>,
    frame: Option<String>,
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "each says something of its own, set by its own side: the terminal's end, the \
              needle, the session's end, a paint due, the window closed by a call"
)]
pub(super) struct Stream {
    filter: Filter,
    /// The ended lines since the output was last taken.
    fresh: Output,
    /// Bytes read since the output was last taken.
    bytes: usize,
    /// Bytes read since the input was typed (or, for a read, not yet taken).
    pub since_write: usize,
    pub changed_at: Instant,
    /// The terminal closed: nothing holds it any more.
    pub eof: bool,
    needle: Option<Vec<u8>>,
    /// The end of what was seen, as long as the needle less one byte: a match split across two
    /// reads is still found.
    carry: Vec<u8>,
    pub matched: bool,
    /// The session ended: the reader stops.
    closed: bool,
    /// The screen, as a terminal shows it: what a window paints, and what a call reads of a
    /// program that draws over its output. Gone once the window says the session ended.
    screen: Option<Screen>,
    /// The screen as a call last read it: a read that finds it the same says so.
    last_screen: Option<String>,
    /// The window that shows the screen, while one is open.
    painter: Option<Arc<Painter>>,
    painted_at: Instant,
    /// Output came since the last paint.
    dirty: bool,
    /// The window was closed by a call: one kept to say the session ended closes too.
    pub dismissed: bool,
}

impl Default for Stream {
    fn default() -> Self {
        Self {
            filter: Filter::default(),
            fresh: Output::default(),
            bytes: 0,
            since_write: 0,
            changed_at: Instant::now(),
            eof: false,
            needle: None,
            carry: vec![],
            matched: false,
            closed: false,
            screen: Some(Screen::new(ROWS, COLUMNS)),
            last_screen: None,
            painter: None,
            painted_at: Instant::now(),
            dirty: false,
            dismissed: false,
        }
    }
}

impl Stream {
    /// Wait for `needle` from now on. Before typing, only what comes after counts; for a read,
    /// the output not yet taken counts too, and so does the line not yet ended, which the next
    /// output goes on (a call may end while `Successfully insta` is printed).
    pub(super) fn expect(&mut self, needle: Option<&str>, typing: bool) {
        // Empty text is in any output: it is no condition at all.
        let needle = needle.filter(|needle| !needle.is_empty());
        self.needle = needle.map(|needle| needle.as_bytes().to_vec());
        self.carry.clear();
        self.matched = false;
        self.since_write = if typing { 0 } else { self.bytes };
        if let (Some(needle), false) = (needle, typing) {
            let pending = self.filter.pending_line();
            let waiting = format!(
                "{}\n{}",
                self.fresh.text(),
                String::from_utf8_lossy(pending)
            );
            self.matched = self.bytes > 0 && waiting.contains(needle);
            // The line's end, as long as the needle less one byte: a match it begins and the
            // next read ends is found by `take_in`, even when the line was taken already.
            let kept = pending.len().min(needle.len() - 1);
            self.carry
                .extend_from_slice(&pending[pending.len() - kept..]);
        }
    }

    /// The output since it was last taken: the ended lines, then the line not yet ended (a
    /// prompt). The pending line stays, so the next ended line still shows the prompt with what
    /// was typed after it, as a terminal's transcript does.
    pub(super) fn take(&mut self) -> Option<String> {
        if self.bytes == 0 {
            return None;
        }
        self.bytes = 0;
        let ended = std::mem::take(&mut self.fresh).text();
        let redrawn = std::mem::take(&mut self.filter.redrawn);
        if let Some(screen) = self.screen.as_ref().filter(|s| redrawn || s.alternate()) {
            let (text, (row, column)) = screen.text();
            let cursor = format!("{COLUMNS}×{ROWS}, cursor at row {row} col {column}");
            if self.last_screen.as_ref() == Some(&text) {
                return Some(format!("screen unchanged ({cursor})"));
            }
            self.last_screen = Some(text.clone());
            return Some(format!("screen {cursor}:\n{text}"));
        }
        let pending = String::from_utf8_lossy(self.filter.pending_line());
        let text = format!("{ended}\n{}", pending.trim_end());
        Some(text.trim().to_string()).filter(|text| !text.is_empty())
    }

    /// The screen now, whatever was read of it before: `screen 120×40, cursor at row R col C:`
    /// and its rows.
    pub(super) fn screen_now(&self) -> String {
        match &self.screen {
            Some(screen) => {
                let (text, (row, column)) = screen.text();
                format!("screen {COLUMNS}×{ROWS}, cursor at row {row} col {column}:\n{text}")
            }
            None => "the session ended: it has no screen".into(),
        }
    }

    /// Take in what the terminal printed; the answers to its queries are returned. A paint it
    /// makes due is the reader's to take ([`Stream::paint_due`]).
    pub(super) fn take_in(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut seen = vec![];
        self.filter.feed(chunk, &mut self.fresh, &mut seen);
        self.bytes += chunk.len();
        self.since_write += chunk.len();
        self.changed_at = Instant::now();
        let mut replies = vec![];
        if let Some(screen) = &mut self.screen {
            screen.feed(chunk);
            // Each query answered where the cursor stands after the whole chunk: close enough
            // for a program that asks once, before it draws.
            let (row, column) = screen.cursor();
            for _ in 0..std::mem::take(&mut self.filter.position_asked) {
                replies.extend_from_slice(format!("\x1b[{row};{column}R").as_bytes());
            }
            self.dirty = true;
        }
        if let (Some(needle), false) = (&self.needle, self.matched) {
            self.carry.extend_from_slice(&seen);
            self.matched = self.carry.windows(needle.len()).any(|at| at == &needle[..]);
            let cut = self.carry.len().saturating_sub(needle.len() - 1);
            self.carry.drain(..cut);
        }
        replies.extend(self.filter.take_replies());
        replies
    }

    pub(super) fn application_cursor(&self) -> bool {
        self.filter.application_cursor
    }

    /// Show the screen in the window `painter` paints, from now on: its first frame. With no
    /// screen, once the session ended, or while a window shows it, the window is let go at once.
    pub(super) fn attach(&mut self, painter: Painter) -> Paint {
        let painter = Arc::new(painter);
        let ended = self.screen.is_none() || self.eof || self.closed;
        if ended || self.painter.is_some() {
            return Paint {
                painter,
                frame: None,
            };
        }
        self.painter = Some(Arc::clone(&painter));
        self.dismissed = false;
        let frame = self.frame();
        Paint { painter, frame }
    }

    /// Close the window, if one is open.
    pub(super) fn detach(&mut self) -> Option<Paint> {
        self.dismissed = true;
        self.let_go()
    }

    /// The window, to be let go once the lock is.
    fn let_go(&mut self) -> Option<Paint> {
        let painter = self.painter.take()?;
        Some(Paint {
            painter,
            frame: None,
        })
    }

    /// The window `painter` was closed by the person: it is painted no more.
    fn gone(&mut self, painter: &Arc<Painter>) {
        if (self.painter.as_ref()).is_some_and(|held| Arc::ptr_eq(held, painter)) {
            self.painter = None;
        }
    }

    pub(super) fn attached(&self) -> bool {
        self.painter.is_some()
    }

    /// How long until a paint is due, while output waits for one.
    pub(super) fn paint_wait(&self) -> Option<Duration> {
        match (self.dirty, &self.painter) {
            (true, Some(_)) => Some(PAINT_EVERY.saturating_sub(self.painted_at.elapsed())),
            _ => None,
        }
    }

    /// What waits, to paint once the lock is let go, if its time came.
    pub(super) fn paint_due(&mut self) -> Option<Paint> {
        if self.paint_wait() != Some(Duration::ZERO) {
            return None;
        }
        let painter = Arc::clone(self.painter.as_ref()?);
        let frame = self.frame();
        Some(Paint { painter, frame })
    }

    /// The terminal closed: its window, if one is open, and the screen it shows, to say so.
    pub(super) fn finish(&mut self) -> Option<(Arc<Painter>, Screen)> {
        self.eof = true;
        let painter = self.painter.take()?;
        Some((painter, self.screen.take()?))
    }

    /// The screen as a frame, painted as of now. A paint made under the lock is the newest by
    /// at least [`PAINT_EVERY`], so paints that run once it is let go come in order.
    fn frame(&mut self) -> Option<String> {
        let frame = self.screen.as_ref()?.frame(None);
        self.painted_at = Instant::now();
        self.dirty = false;
        Some(frame)
    }

    /// The session ended: its window, to be let go once the lock is.
    pub(super) fn close(&mut self) -> Option<Paint> {
        self.closed = true;
        self.let_go()
    }

    pub(super) fn closed(&self) -> bool {
        self.closed
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
