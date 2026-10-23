//! Quitting at the empty prompt: Ctrl-D, or Ctrl-C twice, leaves no prompt row behind.

use std::time::{Duration, Instant};

use crate::common::model::Model;
use crate::common::term::Term;

#[test]
#[allow(
    clippy::disallowed_methods,
    reason = "a test looks at the screen again until it is so, for a bounded time, then gives a \
              late row the time to be drawn: there is no event to wait on"
)]
fn quitting_leaves_no_prompt_row_on_the_screen() {
    for keys in [&["\x04"][..], &["\x03", "\x03"]] {
        let model = Model::start(vec![]);
        let mut term = Term::start(&model, &[]);
        for key in keys {
            term.press(key);
        }
        let end = Instant::now() + Duration::from_secs(5);
        while term.screen().contains('❯') && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(50));
        }
        std::thread::sleep(Duration::from_millis(300));
        let screen = term.screen();
        assert!(!screen.contains('❯'), "{keys:?} left:\n{screen}");
    }
}
