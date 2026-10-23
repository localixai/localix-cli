//! A command's output as the model reads it: the start and the end kept, the middle cut. A build
//! log's first error and its summary line matter most, and memory stays bounded however much a
//! command prints.

use std::collections::VecDeque;

/// Bytes kept from the start, and as many from the end: both streams together stay below what the
/// agent keeps of one result, so nothing is cut a second time.
pub(super) const HALF: usize = 5_000;

#[derive(Debug, Default)]
pub(super) struct Output {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    /// Bytes dropped between the head and the tail.
    dropped: usize,
    /// Characters pushed in all, so the marker can say how many were cut.
    chars: usize,
}

impl Output {
    pub(super) fn push(&mut self, mut bytes: &[u8]) {
        self.chars += characters(bytes);
        let taken = (HALF - self.head.len()).min(bytes.len());
        self.head.extend_from_slice(&bytes[..taken]);
        bytes = &bytes[taken..];
        self.tail.extend(bytes);
        let over = self.tail.len().saturating_sub(HALF);
        self.tail.drain(..over);
        self.dropped += over;
    }

    /// The output as text, trailing whitespace dropped, with a marker where the middle was cut.
    pub(super) fn text(&self) -> String {
        let mut tail = Vec::with_capacity(self.tail.len());
        let (front, back) = self.tail.as_slices();
        tail.extend_from_slice(front);
        tail.extend_from_slice(back);
        if self.dropped == 0 {
            let mut whole = self.head.clone();
            whole.extend_from_slice(&tail);
            return String::from_utf8_lossy(&whole).trim_end().to_string();
        }
        // Cut at character boundaries: the head without a character left unfinished, the tail
        // without the rest of one begun before it.
        let head = match std::str::from_utf8(&self.head) {
            Err(error) if error.error_len().is_none() => &self.head[..error.valid_up_to()],
            _ => &self.head[..],
        };
        let start = tail
            .iter()
            .take(3)
            .take_while(|&&byte| is_continuation(byte))
            .count();
        let tail = &tail[start..];
        let cut = self.chars - characters(head) - characters(tail);
        format!(
            "{}\n... {cut} characters cut ...\n{}",
            String::from_utf8_lossy(head).trim_end(),
            String::from_utf8_lossy(tail).trim_end()
        )
    }
}

/// A UTF-8 byte that continues the character before it.
fn is_continuation(byte: u8) -> bool {
    byte & 0xC0 == 0x80
}

/// Characters in `bytes`: every byte that does not continue one starts one.
fn characters(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&byte| !is_continuation(byte)).count()
}

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
