use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::*;

#[test]
fn text_split_across_reads_is_found() {
    let mut stream = Stream::default();
    stream.expect(Some("ready>"), true);
    stream.take_in(b"not yet\r\nrea");
    assert!(!stream.matched);
    stream.take_in(b"dy> ");
    assert!(stream.matched);
    assert_eq!(stream.take().as_deref(), Some("not yet\nready>"));
}

#[test]
fn only_what_comes_after_the_input_counts() {
    let mut stream = Stream::default();
    stream.take_in(b"done\r\n");
    stream.expect(Some("done"), true);
    assert!(!stream.matched, "the output before the input");
    stream.take_in(b"done again\r\n");
    assert!(stream.matched, "the output after the input");
}

#[test]
fn a_read_counts_the_output_not_yet_taken() {
    let mut stream = Stream::default();
    stream.take_in(b"added 3 packages\r\n");
    stream.expect(Some("added"), false);
    assert!(stream.matched);
    assert_eq!(stream.take().as_deref(), Some("added 3 packages"));
    stream.expect(Some("added"), false);
    assert!(!stream.matched, "taken already");
    stream.expect(Some(""), false);
    assert!(!stream.matched, "empty text is no condition");
}

#[test]
fn a_read_finds_text_the_line_not_yet_ended_begins() {
    let mut stream = Stream::default();
    stream.take_in(b"Successfully insta");
    // An earlier call took the output while the line was being printed.
    assert_eq!(stream.take().as_deref(), Some("Successfully insta"));
    stream.expect(Some("Successfully installed"), false);
    assert!(!stream.matched, "not there yet");
    stream.take_in(b"lled foo\r\n");
    assert!(stream.matched, "the needle began in the pending line");
}

#[test]
fn typing_does_not_count_the_line_it_types_after() {
    let mut stream = Stream::default();
    stream.take_in(b"rea");
    stream.expect(Some("ready"), true);
    stream.take_in(b"dy");
    assert!(!stream.matched, "the needle began before the input");
}

#[test]
fn the_prompt_shows_again_with_what_was_typed() {
    let mut stream = Stream::default();
    stream.take_in(b"Name: ");
    assert_eq!(stream.take().as_deref(), Some("Name:"));
    assert_eq!(stream.take(), None, "nothing new");
    stream.take_in(b"bob\r\nhi bob\r\n");
    assert_eq!(stream.take().as_deref(), Some("Name: bob\nhi bob"));
}

#[test]
fn only_control_sequences_are_no_output() {
    let mut stream = Stream::default();
    stream.take_in(b"\x1b[?25l\x1b[0m");
    assert_eq!(stream.take(), None);
}

#[test]
fn a_query_is_answered_through_the_reader() {
    let mut stream = Stream::default();
    assert_eq!(stream.take_in(b"\x1b[6n"), b"\x1b[1;1R");
    stream.take_in(b"\x1b[?1h");
    assert!(stream.application_cursor());
}

/// A window that records the frames it is given, answers whether it is still open, and counts
/// how often it was closed (dropped).
#[derive(Clone, Default)]
struct Window {
    frames: Arc<Mutex<Vec<String>>>,
    closed_by_person: Arc<AtomicBool>,
    drops: Arc<AtomicUsize>,
}

struct Dropped(Arc<AtomicUsize>);

impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl Window {
    fn painter(&self) -> Painter {
        let (frames, closed) = (Arc::clone(&self.frames), Arc::clone(&self.closed_by_person));
        let dropped = Dropped(Arc::clone(&self.drops));
        Box::new(move |frame| {
            let _ = &dropped;
            frames.lock().unwrap().push(frame);
            !closed.load(Ordering::SeqCst)
        })
    }

    fn paints(&self) -> usize {
        self.frames.lock().unwrap().len()
    }

    fn last(&self) -> String {
        self.frames
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }

    fn drops(&self) -> usize {
        self.drops.load(Ordering::SeqCst)
    }
}

fn watched() -> Stream {
    Stream {
        screen: Some(Screen::new(40, 120)),
        ..Stream::default()
    }
}

#[test]
fn a_window_attached_shows_what_was_printed_before() {
    let (mut stream, window) = (watched(), Window::default());
    stream.take_in(b"before\r\n");
    stream.attach(window.painter());
    assert_eq!(window.paints(), 1);
    assert!(window.last().contains("before"), "{}", window.last());
}

#[test]
fn output_in_a_burst_is_painted_once_its_time_comes() {
    let (mut stream, window) = (watched(), Window::default());
    stream.attach(window.painter());
    stream.take_in(b"one ");
    stream.take_in(b"two");
    assert_eq!(window.paints(), 1, "within the frame's time: not yet");
    let wait = stream.paint_wait().expect("a paint is due");
    assert!(wait <= PAINT_EVERY);
    stream.paint_due();
    assert_eq!(window.paints(), 1, "still too soon");
    std::thread::sleep(PAINT_EVERY);
    assert_eq!(stream.paint_wait(), Some(Duration::ZERO));
    stream.paint_due();
    assert_eq!(window.paints(), 2);
    assert!(window.last().contains("one two"), "{}", window.last());
    assert_eq!(stream.paint_wait(), None, "nothing new");
    // Once the frame's time passed, the next output is painted at once.
    std::thread::sleep(PAINT_EVERY);
    stream.take_in(b"!");
    assert_eq!(window.paints(), 3);
}

#[test]
fn a_window_the_person_closed_is_painted_no_more() {
    let (mut stream, window) = (watched(), Window::default());
    window.closed_by_person.store(true, Ordering::SeqCst);
    stream.attach(window.painter());
    assert_eq!(window.drops(), 1, "let go at once");
    assert!(!stream.attached());
    std::thread::sleep(PAINT_EVERY);
    stream.take_in(b"more");
    assert_eq!(window.paints(), 1);
}

#[test]
fn no_window_opens_on_an_ended_session() {
    for end in [true, false] {
        let (mut stream, window) = (watched(), Window::default());
        match end {
            true => assert!(stream.finish().is_none(), "no window yet"),
            false => stream.close(),
        }
        stream.attach(window.painter());
        assert_eq!((window.paints(), window.drops()), (0, 1), "eof: {end}");
    }
}

#[test]
fn closing_the_session_closes_its_window_once() {
    let (mut stream, window) = (watched(), Window::default());
    stream.attach(window.painter());
    stream.close();
    stream.close();
    assert_eq!(window.drops(), 1);
}

#[test]
fn a_detached_window_closes_and_the_end_keeps_nothing() {
    let (mut stream, window) = (watched(), Window::default());
    stream.attach(window.painter());
    stream.detach();
    assert_eq!(window.drops(), 1);
    assert!(stream.finish().is_none());
}

#[test]
fn the_end_hands_over_the_window_and_its_screen() {
    let (mut stream, window) = (watched(), Window::default());
    stream.attach(window.painter());
    stream.take_in(b"bye");
    let (painter, screen) = stream.finish().expect("the window, to say it ended");
    assert!(stream.eof);
    assert!(painter(screen.frame(Some(" · exited 0"))));
    assert!(window.last().contains("bye") && window.last().contains("exited 0"));
    drop(painter);
    assert_eq!(window.drops(), 1);
}

#[test]
fn without_a_screen_nothing_is_painted_and_nothing_changes() {
    let (mut stream, window) = (Stream::default(), Window::default());
    stream.attach(window.painter());
    assert_eq!((window.paints(), window.drops()), (0, 1));
    assert_eq!(stream.take_in(b"\x1b[6nhi"), b"\x1b[1;1R");
    assert_eq!(stream.take().as_deref(), Some("hi"));
    assert_eq!(stream.paint_wait(), None);
}
