use std::io::Read;

use super::*;

/// A frame is one buffer, and it reaches the terminal in one write: nothing of it (its last row,
/// the cursor's place, the update's end) waits for a flush, as std's line-buffered stdout keeps
/// what follows a frame's last line break for a second write.
#[cfg(unix)]
#[test]
fn a_frame_reaches_the_terminal_whole_before_any_flush() {
    let (mut terminal, end) = io::pipe().unwrap();
    let mut out = unbuffered(&end).unwrap();
    let frame = "\x1b[?2026h\r\x1b[Jdone\r\n❯ typing\x1b[?25h\x1b[?2026l".as_bytes();
    out.write_all(frame).unwrap();
    let mut read = vec![0; 256];
    let got = terminal.read(&mut read).unwrap();
    assert_eq!(&read[..got], frame);
}
