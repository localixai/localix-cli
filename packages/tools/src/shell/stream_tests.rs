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

/// `window` shows the stream from now on: its first frame, painted once the lock is let go.
fn attach(shared: &Shared, window: &Window) {
    let first = shared.lock().attach(window.painter());
    shared.tell(Some(first));
}

/// What the terminal printed, taken in and painted when a paint is due, as the reader does.
fn print(shared: &Shared, chunk: &[u8]) {
    let paint = {
        let mut stream = shared.lock();
        stream.take_in(chunk);
        stream.paint_due()
    };
    shared.tell(paint);
}

fn paint_due(shared: &Shared) {
    let paint = shared.lock().paint_due();
    shared.tell(paint);
}

#[test]
fn a_window_attached_shows_what_was_printed_before() {
    let (shared, window) = (Shared::default(), Window::default());
    print(&shared, b"before\r\n");
    let first = shared.lock().attach(window.painter());
    assert_eq!(window.paints(), 0, "not under the lock");
    shared.tell(Some(first));
    assert_eq!(window.paints(), 1);
    assert!(window.last().contains("before"), "{}", window.last());
}

#[expect(
    clippy::disallowed_methods,
    reason = "the frame's time passes: the paint's clock is what is tested"
)]
#[test]
fn output_in_a_burst_is_painted_once_its_time_comes() {
    let (shared, window) = (Shared::default(), Window::default());
    attach(&shared, &window);
    print(&shared, b"one ");
    print(&shared, b"two");
    assert_eq!(window.paints(), 1, "within the frame's time: not yet");
    let wait = shared.lock().paint_wait().expect("a paint is due");
    assert!(wait <= PAINT_EVERY);
    paint_due(&shared);
    assert_eq!(window.paints(), 1, "still too soon");
    std::thread::sleep(PAINT_EVERY);
    assert_eq!(shared.lock().paint_wait(), Some(Duration::ZERO));
    paint_due(&shared);
    assert_eq!(window.paints(), 2);
    assert!(window.last().contains("one two"), "{}", window.last());
    assert_eq!(shared.lock().paint_wait(), None, "nothing new");
    // Once the frame's time passed, the next output is painted at once.
    std::thread::sleep(PAINT_EVERY);
    print(&shared, b"!");
    assert_eq!(window.paints(), 3);
}

#[expect(
    clippy::disallowed_methods,
    reason = "the frame's time passes: the paint's clock is what is tested"
)]
#[test]
fn a_window_the_person_closed_is_painted_no_more() {
    let (shared, window) = (Shared::default(), Window::default());
    window.closed_by_person.store(true, Ordering::SeqCst);
    attach(&shared, &window);
    assert_eq!(window.drops(), 1, "let go at once");
    assert!(!shared.lock().attached());
    std::thread::sleep(PAINT_EVERY);
    print(&shared, b"more");
    assert_eq!(window.paints(), 1);
}

#[test]
fn no_window_opens_on_an_ended_session() {
    for end in [true, false] {
        let (shared, window) = (Shared::default(), Window::default());
        match end {
            true => assert!(shared.lock().finish().is_none(), "no window yet"),
            false => {
                let gone = shared.lock().close();
                shared.tell(gone);
            }
        }
        attach(&shared, &window);
        assert_eq!((window.paints(), window.drops()), (0, 1), "eof: {end}");
    }
}

#[test]
fn a_shown_session_keeps_its_window() {
    let (shared, shown, second) = (Shared::default(), Window::default(), Window::default());
    attach(&shared, &shown);
    attach(&shared, &second);
    assert_eq!((second.paints(), second.drops()), (0, 1));
    assert_eq!((shown.paints(), shown.drops()), (1, 0));
}

#[test]
fn closing_the_session_closes_its_window_once() {
    let (shared, window) = (Shared::default(), Window::default());
    attach(&shared, &window);
    for _ in 0..2 {
        let gone = shared.lock().close();
        shared.tell(gone);
    }
    assert_eq!(window.drops(), 1);
}

#[test]
fn a_detached_window_closes_and_the_end_keeps_nothing() {
    let (shared, window) = (Shared::default(), Window::default());
    attach(&shared, &window);
    let gone = shared.lock().detach();
    assert_eq!(window.drops(), 0, "not under the lock");
    shared.tell(gone);
    assert_eq!(window.drops(), 1);
    assert!(shared.lock().finish().is_none());
}

#[test]
fn the_end_hands_over_the_window_and_its_screen() {
    let (shared, window) = (Shared::default(), Window::default());
    attach(&shared, &window);
    print(&shared, b"bye");
    let (painter, screen) = (shared.lock().finish()).expect("the window, to say it ended");
    assert!(shared.lock().eof);
    assert!(painter(screen.frame(Some(" · exited 0"))));
    assert!(window.last().contains("bye") && window.last().contains("exited 0"));
    drop(painter);
    assert_eq!(window.drops(), 1);
}

#[test]
fn a_full_screen_program_is_read_as_its_screen() {
    let mut stream = Stream::default();
    stream.take_in(b"\x1b[?1049h\x1b[H\x1b[2Jhello\x1b[5;3Hworld");
    let read = stream.take().unwrap();
    let mut lines = read.lines();
    assert_eq!(
        lines.next(),
        Some("screen 120×40, cursor at row 5 col 8:"),
        "{read}"
    );
    assert_eq!(lines.collect::<Vec<_>>(), ["hello", "", "", "", "  world"]);
}

#[test]
fn a_program_that_redraws_in_place_is_read_as_its_screen() {
    // As an agent's interface draws: no alternate screen, but the cursor goes up and lines are erased.
    let mut stream = Stream::default();
    stream.take_in(b"> hi\r\n* thinking\r\n");
    stream.take_in(b"\x1b[1A\x1b[2K\x1b[Ganswer: 42\r\n");
    let read = stream.take().unwrap();
    assert!(read.starts_with("screen 120×40"), "{read}");
    assert!(read.contains("> hi\nanswer: 42"), "{read}");
    assert!(!read.contains("thinking"), "{read}");
}

#[test]
fn a_screen_read_again_unchanged_says_so() {
    let mut stream = Stream::default();
    stream.take_in(b"\x1b[?1049h\x1b[Hmenu");
    stream.take();
    // A redraw of the same thing (a cursor blink, a timer that did not move).
    stream.take_in(b"\x1b[Hmenu");
    assert_eq!(
        stream.take().as_deref(),
        Some("screen unchanged (120×40, cursor at row 1 col 5)")
    );
}

#[test]
fn plain_output_stays_a_transcript() {
    let mut stream = Stream::default();
    stream.take_in(b"\x1b[32mok\x1b[0m 1\r\nok 2\r\n>>> ");
    assert_eq!(stream.take().as_deref(), Some("ok 1\nok 2\n>>>"));
}

#[test]
fn where_the_cursor_is_is_answered_from_the_screen() {
    let mut stream = Stream::default();
    assert_eq!(stream.take_in(b"\r\nabc\x1b[6n"), b"\x1b[2;4R");
}
