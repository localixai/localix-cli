//! The person's terminal colours, which the windows of terminal sessions take: the text, the
//! background and the cursor, asked of the terminal once (OSC 10, 11 and 12). A query every
//! terminal answers (the primary device attributes) follows them, so a terminal that knows none
//! of them costs no wait; one that answers nothing is waited on briefly. Nothing is written when
//! llx is not in a terminal.

use std::fs::File;
use std::io::{IsTerminal, Read, Write};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use tools::shell::{Rgb, Theme};

/// The three colours, each ended by ST, then the device attributes.
const QUERY: &[u8] = b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]12;?\x1b\\\x1b[c";
/// The longest wait for the answers: a terminal over a slow link may miss it, and the windows
/// then take the system's light or dark.
const WAIT: Duration = Duration::from_millis(150);

/// Where the questions go and the answers come from.
pub(crate) trait Terminal {
    /// Whether all of `bytes` were written.
    fn write(&mut self, bytes: &[u8]) -> bool;
    /// What the terminal sent within `wait`, or nothing.
    fn read(&mut self, wait: Duration) -> Option<Vec<u8>>;
}

/// The colours of llx's own terminal, or none. Asked before the interface reads the keys, since
/// that reader would take the answers.
pub(crate) fn ask() -> Option<Theme> {
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        return None;
    }
    let file = File::options().read(true).write(true).open("/dev/tty");
    let mut tty = Tty(file.ok()?);
    // Raw, so the answers are not echoed and come without waiting for a line.
    let raw = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
    if !raw {
        crossterm::terminal::enable_raw_mode().ok()?;
    }
    let theme = query(&mut tty, WAIT);
    if !raw {
        let _ = crossterm::terminal::disable_raw_mode();
    }
    theme
}

/// Ask `terminal` for its colours, waiting at most `wait` for the answers.
pub(crate) fn query(terminal: &mut dyn Terminal, wait: Duration) -> Option<Theme> {
    if !terminal.write(QUERY) {
        return None;
    }
    let deadline = Instant::now() + wait;
    let mut reply = vec![];
    while !answered(&reply) {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match terminal.read(left) {
            Some(bytes) => reply.extend_from_slice(&bytes),
            None => break,
        }
    }
    let fg = colour(&reply, b"10")?;
    Some(Theme {
        fg,
        bg: colour(&reply, b"11")?,
        cursor: colour(&reply, b"12").unwrap_or(fg),
    })
}

/// Whether the device attributes came: `ESC [ ?` up to a `c`, the last answer.
fn answered(reply: &[u8]) -> bool {
    find(reply, b"\x1b[?").is_some_and(|at| reply[at..].contains(&b'c'))
}

/// The colour OSC `code` answered, as `rgb:R/G/B` with one to four hex digits each.
fn colour(reply: &[u8], code: &[u8]) -> Option<Rgb> {
    let head = [b"\x1b]", code, b";rgb:"].concat();
    let start = find(reply, &head)? + head.len();
    let value = &reply[start..];
    let end = value
        .iter()
        .position(|&byte| byte == 0x07 || byte == 0x1b)?;
    let text = std::str::from_utf8(&value[..end]).ok()?;
    let parts: Vec<&str> = text.split('/').collect();
    let [red, green, blue] = parts[..] else {
        return None;
    };
    Some([channel(red)?, channel(green)?, channel(blue)?])
}

/// A channel of 1 to 4 hex digits, scaled to 8 bits.
fn channel(digits: &str) -> Option<u8> {
    if digits.is_empty() || digits.len() > 4 {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    let max = (1u32 << (4 * digits.len())) - 1;
    u8::try_from((value * 255 + max / 2) / max).ok()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The terminal llx runs in.
struct Tty(File);

impl Terminal for Tty {
    fn write(&mut self, bytes: &[u8]) -> bool {
        self.0
            .write_all(bytes)
            .and_then(|()| self.0.flush())
            .is_ok()
    }

    fn read(&mut self, wait: Duration) -> Option<Vec<u8>> {
        // select, not poll: macOS's poll does not wait on a terminal device, and says it is
        // ready at once, so the read would block until the person typed.
        let fd = self.0.as_raw_fd();
        let mut timeout = libc::timeval {
            tv_sec: wait.as_secs() as libc::time_t,
            tv_usec: wait.subsec_micros() as libc::suseconds_t,
        };
        // SAFETY: the set and the timeout live through the call, and `fd` is open and below
        // FD_SETSIZE (a process this young has few descriptors).
        let ready = unsafe {
            let mut set: libc::fd_set = std::mem::zeroed();
            libc::FD_SET(fd, &mut set);
            libc::select(
                fd + 1,
                &mut set,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut timeout,
            ) > 0
                && libc::FD_ISSET(fd, &set)
        };
        if !ready {
            return None;
        }
        let mut buffer = [0; 256];
        let read = self.0.read(&mut buffer).ok().filter(|&read| read > 0)?;
        Some(buffer[..read].to_vec())
    }
}

#[cfg(test)]
#[path = "colours_tests.rs"]
mod tests;
