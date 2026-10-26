//! A terminal's output as a transcript: control sequences dropped, a carriage return's redraw kept
//! as its last state, and the few queries a program waits on answered. It is a byte state machine
//! whose state carries over between reads, so a sequence split across two reads is still dropped.
//! There is no screen: a full-screen program shows only fragments of its text.

use super::output::{HALF, Output};

/// Parameter bytes of a CSI sequence kept: the ones that matter are a few bytes long.
const PARAMETERS: usize = 16;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum State {
    #[default]
    Ground,
    /// After ESC.
    Escape,
    /// After `ESC [`, up to the final byte.
    Csi,
    /// In an OSC, DCS or another string sequence, up to BEL or `ESC \`.
    Text,
    /// ESC inside a string sequence: `\` ends it.
    TextEscape,
    /// After `ESC (` and its kin: one more byte names a character set.
    Charset,
}

#[derive(Debug, Default)]
pub(super) struct Filter {
    state: State,
    parameters: Vec<u8>,
    /// The line not yet ended: a prompt waits here.
    line: Vec<u8>,
    /// A lone carriage return came: the next character starts the line over.
    at_cr: bool,
    /// The program turned on application cursor keys (DECCKM): arrows are sent as SS3.
    pub(super) application_cursor: bool,
    /// Answers to the program's queries, for the terminal's input.
    replies: Vec<u8>,
    /// The program asked where the cursor is: the screen answers.
    pub(super) position_asked: usize,
    /// The program drew over what it printed (the cursor up or placed, the screen erased, the
    /// alternate screen): a transcript would hold fragments, so its screen is read instead.
    pub(super) redrawn: bool,
}

impl Filter {
    /// Take in `bytes`: each ended line goes to `out`, and every character kept (and each line
    /// break) to `seen`, where a wait looks for its text.
    pub(super) fn feed(&mut self, bytes: &[u8], out: &mut Output, seen: &mut Vec<u8>) {
        for &byte in bytes {
            self.byte(byte, out, seen);
        }
    }

    /// The line not yet ended, such as a `Name: ` prompt.
    pub(super) fn pending_line(&self) -> &[u8] {
        &self.line
    }

    /// The answers queued since the last call, to write to the program.
    pub(super) fn take_replies(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.replies)
    }

    fn byte(&mut self, byte: u8, out: &mut Output, seen: &mut Vec<u8>) {
        match self.state {
            State::Ground => self.ground(byte, out, seen),
            State::Escape => {
                self.state = match byte {
                    b'[' => {
                        self.parameters.clear();
                        State::Csi
                    }
                    b']' | b'P' | b'X' | b'^' | b'_' => State::Text,
                    b'(' | b')' | b'*' | b'+' => State::Charset,
                    0x1b => State::Escape,
                    _ => State::Ground,
                }
            }
            State::Csi => match byte {
                0x40..=0x7e => {
                    self.csi(byte);
                    self.state = State::Ground;
                }
                0x1b => self.state = State::Escape,
                _ if self.parameters.len() < PARAMETERS => self.parameters.push(byte),
                _ => {}
            },
            State::Text => match byte {
                0x07 => self.state = State::Ground,
                0x1b => self.state = State::TextEscape,
                _ => {}
            },
            State::TextEscape => match byte {
                b'\\' => self.state = State::Ground,
                // Not a string terminator: the ESC began a sequence of its own.
                _ => {
                    self.state = State::Escape;
                    self.byte(byte, out, seen);
                }
            },
            State::Charset => self.state = State::Ground,
        }
    }

    fn ground(&mut self, byte: u8, out: &mut Output, seen: &mut Vec<u8>) {
        match byte {
            b'\n' => {
                self.line.push(b'\n');
                out.push(&self.line);
                self.line.clear();
                self.at_cr = false;
                seen.push(b'\n');
            }
            b'\r' => self.at_cr = true,
            0x08 => {
                // The last character, however many bytes it took.
                while let Some(last) = self.line.pop() {
                    if last & 0xC0 != 0x80 {
                        break;
                    }
                }
            }
            0x1b => self.state = State::Escape,
            b'\t' | 0x20.. if byte != 0x7f => {
                if self.at_cr {
                    // A redraw (a progress bar's `\r 45%`): only its last state is kept.
                    self.line.clear();
                    self.at_cr = false;
                }
                self.line.push(byte);
                seen.push(byte);
                if self.line.len() >= HALF {
                    // A line with no end in sight: kept as it is, so memory stays bounded.
                    out.push(&self.line);
                    self.line.clear();
                }
            }
            // BEL and the other controls do nothing a transcript shows.
            _ => {}
        }
    }

    /// The side effects of a CSI sequence ending in `last`; the sequence itself is dropped.
    fn csi(&mut self, last: u8) {
        let parameters = self.parameters.as_slice();
        match (parameters, last) {
            ([b'?', modes @ ..], b'h' | b'l') => {
                let modes = || modes.split(|&b| b == b';');
                if modes().any(|m| m == b"1") {
                    self.application_cursor = last == b'h';
                }
                if last == b'h' && modes().any(|m| [&b"1049"[..], b"1047", b"47"].contains(&m)) {
                    self.redrawn = true;
                }
            }
            (_, b'A' | b'H' | b'f' | b'J' | b'd') => self.redrawn = true,
            // Where the cursor is, and whether the terminal is well: a program that asks waits
            // for the answer (the screen knows where the cursor is).
            (b"6", b'n') => self.position_asked += 1,
            (b"5", b'n') => self.replies.extend_from_slice(b"\x1b[0n"),
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "filter_tests.rs"]
mod tests;
