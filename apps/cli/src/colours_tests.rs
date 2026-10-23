use std::collections::VecDeque;

use super::*;

/// A terminal that answers with scripted reads, and records what was written to it.
#[derive(Default)]
struct Fake {
    written: Vec<u8>,
    replies: VecDeque<Vec<u8>>,
    /// Whether writing fails, as on a closed terminal.
    broken: bool,
    /// How long it was waited on, in all.
    waited: Duration,
}

impl Fake {
    fn answering(replies: &[&[u8]]) -> Self {
        Self {
            replies: replies.iter().map(|reply| reply.to_vec()).collect(),
            ..Self::default()
        }
    }
}

impl Terminal for Fake {
    fn write(&mut self, bytes: &[u8]) -> bool {
        self.written.extend_from_slice(bytes);
        !self.broken
    }

    fn read(&mut self, wait: Duration) -> Option<Vec<u8>> {
        let reply = self.replies.pop_front();
        if reply.is_none() {
            self.waited += wait;
        }
        reply
    }
}

const WAIT: Duration = Duration::from_millis(150);
const DA1: &[u8] = b"\x1b[?62;22c";

#[test]
fn the_terminal_s_colours_are_read_from_its_answers() {
    let mut terminal = Fake::answering(&[
        b"\x1b]10;rgb:ffff/8080/0000\x1b\\",
        b"\x1b]11;rgb:1e1e/1e1e/1e1e\x07\x1b]12;rgb:00/ff/80\x1b\\",
        DA1,
    ]);
    let theme = query(&mut terminal, WAIT);
    assert_eq!(
        theme,
        Some(Theme {
            fg: [0xff, 0x80, 0x00],
            bg: [0x1e, 0x1e, 0x1e],
            cursor: [0x00, 0xff, 0x80],
        })
    );
    assert_eq!(terminal.written, QUERY);
    assert_eq!(
        terminal.waited,
        Duration::ZERO,
        "the last answer ends the wait"
    );
}

#[test]
fn answers_cut_anywhere_are_put_together() {
    let whole: &[u8] = b"\x1b]10;rgb:f/8/0\x07\x1b]11;rgb:fff/000/888\x07\x1b[?1;2c";
    let pieces: Vec<&[u8]> = whole.chunks(3).collect();
    let theme = query(&mut Fake::answering(&pieces), WAIT).unwrap();
    assert_eq!(theme.fg, [0xff, 0x88, 0x00]);
    assert_eq!(theme.bg, [0xff, 0x00, 0x88]);
    assert_eq!(theme.cursor, theme.fg, "no cursor colour: the text's");
}

#[test]
fn a_terminal_that_knows_none_of_them_gives_none_at_once() {
    let mut terminal = Fake::answering(&[DA1]);
    assert_eq!(query(&mut terminal, WAIT), None);
    assert_eq!(terminal.waited, Duration::ZERO);
}

#[test]
fn a_silent_terminal_is_waited_on_briefly() {
    let mut terminal = Fake::default();
    assert_eq!(query(&mut terminal, WAIT), None);
    assert!(terminal.waited <= WAIT, "{:?}", terminal.waited);
}

#[test]
fn a_broken_terminal_or_answer_gives_none() {
    let mut broken = Fake {
        broken: true,
        ..Fake::answering(&[b"\x1b]10;rgb:ff/ff/ff\x07\x1b]11;rgb:00/00/00\x07", DA1])
    };
    assert_eq!(query(&mut broken, WAIT), None);
    for reply in [
        &b"\x1b]10;rgb:ff/ff\x07\x1b]11;rgb:00/00/00\x07"[..],
        b"\x1b]10;rgb:zz/ff/ff\x07\x1b]11;rgb:00/00/00\x07",
        b"\x1b]10;rgb:fffff/ff/ff\x07\x1b]11;rgb:00/00/00\x07",
        b"\x1b]10;rgb:ff/ff/ff\x07",
    ] {
        let mut terminal = Fake::answering(&[reply, DA1]);
        assert_eq!(query(&mut terminal, WAIT), None, "{reply:?}");
    }
}
