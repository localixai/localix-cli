//! A command's output as the model reads it: the start and the end kept, the middle cut. A build
//! log's first error and its summary line matter most, and memory stays bounded however much a
//! command prints.

/// Bytes kept from the start, and from the end: together they stay below what the agent keeps of
/// one result, so nothing is cut a second time. The end is the longer: a build log's summary and
/// the last error come last, and the start only says what ran.
pub(super) const HEAD: usize = 8_000;
pub(super) const TAIL: usize = 32_000;

#[derive(Debug, Default)]
pub(super) struct Output {
    head: Vec<u8>,
    /// The latest bytes, trimmed to [`TAIL`] only when twice that is held, so a push is a slice
    /// copy and the front is dropped once in a while.
    tail: Vec<u8>,
    /// Bytes already dropped from the tail's front.
    dropped: usize,
    /// Characters pushed in all, so the marker can say how many were cut.
    chars: usize,
}

impl Output {
    pub(super) fn push(&mut self, mut bytes: &[u8]) {
        self.chars += characters(bytes);
        let taken = (HEAD - self.head.len()).min(bytes.len());
        let (first, rest) = bytes.split_at(taken);
        self.head.extend_from_slice(first);
        bytes = rest;
        self.tail.extend_from_slice(bytes);
        if self.tail.len() > 2 * TAIL {
            let over = self.tail.len() - TAIL;
            self.tail.drain(..over);
            self.dropped += over;
        }
    }

    /// The output as text, trailing whitespace dropped, with a marker where the middle was cut.
    pub(super) fn text(&self) -> String {
        let over = self.tail.len().saturating_sub(TAIL);
        let tail = self.tail.get(over..).unwrap_or_default();
        if self.dropped + over == 0 {
            let mut whole = self.head.clone();
            whole.extend_from_slice(tail);
            return String::from_utf8_lossy(&whole).trim_end().to_string();
        }
        let (head, tail) = (whole_lines_end(&self.head), whole_lines_start(tail));
        let cut = self.chars - characters(head) - characters(tail);
        format!(
            "{}\n... {cut} characters cut; redirect the output to a file and read it in parts \
             ...\n{}",
            String::from_utf8_lossy(head).trim_end(),
            String::from_utf8_lossy(tail).trim_end()
        )
    }
}

/// The start of a stream cut after its last complete line, else at a character boundary.
fn whole_lines_end(head: &[u8]) -> &[u8] {
    if let Some(end) = head.iter().rposition(|&byte| byte == b'\n') {
        return head.get(..=end).unwrap_or(head);
    }
    match std::str::from_utf8(head) {
        Err(error) if error.error_len().is_none() => {
            head.get(..error.valid_up_to()).unwrap_or(head)
        }
        _ => head,
    }
}

/// The end of a stream cut before its first complete line, else after the rest of a character
/// begun before it.
fn whole_lines_start(tail: &[u8]) -> &[u8] {
    if let Some(start) = tail.iter().position(|&byte| byte == b'\n') {
        return tail.get(start + 1..).unwrap_or(tail);
    }
    let skip = tail
        .iter()
        .take(3)
        .take_while(|&&byte| is_continuation(byte))
        .count();
    tail.get(skip..).unwrap_or(tail)
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
