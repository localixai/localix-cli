//! A child process the interface waits for, with a deadline (R10): its output read on a thread
//! of its own, its input written on another, and past the deadline it is killed.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// What `command` printed, if it ended well within `wait`; past it, it is killed. `input` is
/// what it is given to read. What is waited for is the output's end.
pub(super) fn within(mut command: Command, input: Vec<u8>, wait: Duration) -> Option<Vec<u8>> {
    let mut child = (command.stdin(Stdio::piped()))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if let Some(mut stdin) = child.stdin.take() {
        // On a thread of its own: a child that never reads must not hold the caller.
        let writer = thread::Builder::new().name("llx-child-in".into());
        let _ = writer.spawn(move || {
            let _ = stdin.write_all(&input);
        });
    }
    let printed = child.stdout.take().and_then(|out| {
        let (read, done) = mpsc::sync_channel(1);
        let reader = thread::Builder::new().name("llx-child-out".into());
        let reading = reader.spawn(move || {
            let mut bytes = vec![];
            let _ = out.take(64 << 20).read_to_end(&mut bytes);
            let _ = read.send(bytes);
        });
        reading.ok().and_then(|_| done.recv_timeout(wait).ok())
    });
    let Some(printed) = printed else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    // Its output ended as it exited: the status is there to be read.
    let ended = child.wait().ok()?;
    ended.success().then_some(printed)
}

#[cfg(test)]
#[path = "within_tests.rs"]
mod tests;
