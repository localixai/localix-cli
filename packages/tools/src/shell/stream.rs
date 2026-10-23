//! What a terminal printed, as a call reads it: the transcript since the last call, the line a
//! prompt waits on, and whether the text a call waits for has appeared. The reader thread fills
//! it; a call takes from it. Nothing is lost between calls: what comes while no call waits stays
//! until the next one.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use super::filter::Filter;
use super::output::Output;
use super::screen::Screen;

/// A window's painter: a frame in, whether the window is still open out.
pub type Painter = Box<dyn Fn(String) -> bool + Send>;
pub const PAINT_EVERY: Duration = Duration::from_millis(33);

/// The output read so far, and the wake for who waits on it.
#[derive(Default)]
pub struct Shared {
    state: Mutex<Stream>,
    pub wake: Condvar,
}

impl Shared {
    /// Shared output, with `screen` kept for a window when there is one.
    pub fn new(screen: Option<Screen>) -> Self {
        Self {
            state: Mutex::new(Stream {
                screen,
                ..Stream::default()
            }),
            wake: Condvar::new(),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, Stream> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

pub struct Stream {
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
    /// The screen, kept only for a window: without one, the output is never parsed twice.
    screen: Option<Screen>,
    /// The window that shows the screen, while one is open.
    painter: Option<Painter>,
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
            screen: None,
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
    pub fn expect(&mut self, needle: Option<&str>, typing: bool) {
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
    pub fn take(&mut self) -> Option<String> {
        if self.bytes == 0 {
            return None;
        }
        self.bytes = 0;
        let ended = std::mem::take(&mut self.fresh).text();
        let pending = String::from_utf8_lossy(self.filter.pending_line());
        let text = format!("{ended}\n{}", pending.trim_end());
        Some(text.trim().to_string()).filter(|text| !text.is_empty())
    }

    /// Take in what the terminal printed; the answers to its queries are returned.
    pub fn take_in(&mut self, chunk: &[u8]) -> Vec<u8> {
        let mut seen = vec![];
        self.filter.feed(chunk, &mut self.fresh, &mut seen);
        self.bytes += chunk.len();
        self.since_write += chunk.len();
        self.changed_at = Instant::now();
        if let Some(screen) = &mut self.screen {
            screen.feed(chunk);
            self.dirty = true;
            self.paint_due();
        }
        if let (Some(needle), false) = (&self.needle, self.matched) {
            self.carry.extend_from_slice(&seen);
            self.matched = self.carry.windows(needle.len()).any(|at| at == &needle[..]);
            let cut = self.carry.len().saturating_sub(needle.len() - 1);
            self.carry.drain(..cut);
        }
        self.filter.take_replies()
    }

    pub fn application_cursor(&self) -> bool {
        self.filter.application_cursor
    }

    /// Show the screen in the window `painter` paints, from now on. With no screen, or once the
    /// session ended, the window is let go at once.
    pub fn attach(&mut self, painter: Painter) {
        if self.screen.is_none() || self.eof || self.closed {
            return;
        }
        self.painter = Some(painter);
        self.dismissed = false;
        self.paint();
    }

    /// Close the window, if one is open.
    pub fn detach(&mut self) {
        self.painter = None;
        self.dismissed = true;
    }

    pub fn attached(&self) -> bool {
        self.painter.is_some()
    }

    /// How long until a paint is due, while output waits for one.
    pub fn paint_wait(&self) -> Option<Duration> {
        match (self.dirty, &self.painter) {
            (true, Some(_)) => Some(PAINT_EVERY.saturating_sub(self.painted_at.elapsed())),
            _ => None,
        }
    }

    /// Paint what waits, if its time came.
    pub fn paint_due(&mut self) {
        if self.paint_wait() == Some(Duration::ZERO) {
            self.paint();
        }
    }

    /// The terminal closed: its window, if one is open, and the screen it shows, to say so.
    pub fn finish(&mut self) -> Option<(Painter, Screen)> {
        self.eof = true;
        let painter = self.painter.take()?;
        Some((painter, self.screen.take()?))
    }

    fn paint(&mut self) {
        let (Some(painter), Some(screen)) = (&self.painter, &self.screen) else {
            return;
        };
        if !painter(screen.frame(None)) {
            self.painter = None;
        }
        self.painted_at = Instant::now();
        self.dirty = false;
    }

    pub fn close(&mut self) {
        self.closed = true;
        self.painter = None;
    }

    pub fn closed(&self) -> bool {
        self.closed
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
