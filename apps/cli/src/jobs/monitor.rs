//! A monitor's command: run in the background, in a session of its own (so a stop kills
//! everything it started), with no input. The lines it prints come back in batches: those that
//! arrive close together make one message, so a burst is one step, not many.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::Stdio;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// How long a batch waits for more lines after its first.
const GATHER: Duration = Duration::from_millis(400);
/// Lines in one message at most, and characters in one line.
const LINES: usize = 40;
const LINE_CHARS: usize = 400;
/// How often the stop and the deadline are looked at.
const TICK: Duration = Duration::from_millis(100);

/// How a watch ended.
pub(super) enum Ended {
    Exit(String),
    Deadline,
    Stopped,
}

/// Run `command` in `dir` with `env` until it exits, `deadline` passes or `stopped` says so; give each
/// batch of lines to `lines`.
pub(super) fn watch(
    command: &str,
    dir: &Path,
    env: tools::shell::Env,
    deadline: Instant,
    stopped: &dyn Fn() -> bool,
    lines: &dyn Fn(Vec<String>, usize),
) -> Result<Ended, String> {
    // The tools' own way to run a command: a session of its own, only the standard streams
    // inherited (a monitor lives long: a descriptor inherited by chance would stay open as long).
    let mut shell = tools::shell::shell(command);
    shell
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    env.apply(&mut shell);
    let mut child = shell
        .spawn()
        .map_err(|error| format!("the command could not start: {error}"))?;
    // The loop below takes each line as it comes; a burst beyond this waits in the pipe.
    let (sent, received) = mpsc::sync_channel(LINES);
    for stream in [
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let sent = sent.clone();
        let reader = std::thread::Builder::new().name("llx-monitor".into());
        let started = reader.spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if sent.send(line).is_err() {
                    return;
                }
            }
        });
        // No thread to read it: the stream is dropped with the closure, and the command sees
        // it closed.
        drop(started);
    }
    drop(sent);
    let mut batch: Vec<String> = vec![];
    let mut dropped = 0;
    let mut since: Option<Instant> = None;
    let flush = |batch: &mut Vec<String>, dropped: &mut usize| {
        if !batch.is_empty() {
            lines(std::mem::take(batch), std::mem::take(dropped));
        }
    };
    // Both streams closed: the command is done, or about to be, or went on without them.
    let mut closed = false;
    let ended = loop {
        if stopped() {
            break Ended::Stopped;
        }
        if Instant::now() >= deadline {
            break Ended::Deadline;
        }
        if closed {
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                break Ended::Exit(match status.code() {
                    Some(code) => format!("exit {code}"),
                    None => "killed by a signal".into(),
                });
            }
            // Nothing says when a child exits: looked at as often as the stop is below.
            #[expect(
                clippy::disallowed_methods,
                reason = "a poll's backoff, as recv_timeout's"
            )]
            std::thread::sleep(TICK);
            continue;
        }
        match received.recv_timeout(TICK) {
            Ok(line) => {
                since.get_or_insert_with(Instant::now);
                match batch.len() < LINES {
                    true => batch.push(line.chars().take(LINE_CHARS).collect()),
                    false => dropped += 1,
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                flush(&mut batch, &mut dropped);
                closed = true;
            }
        }
        if since.is_some_and(|at| at.elapsed() >= GATHER) {
            flush(&mut batch, &mut dropped);
            since = None;
        }
    };
    flush(&mut batch, &mut dropped);
    if !matches!(ended, Ended::Exit(_)) {
        tools::shell::kill(&mut child);
    }
    Ok(ended)
}

#[cfg(test)]
#[path = "monitor_tests.rs"]
mod tests;
