//! A monitor's command: run in the background, in a process group of its own (so a stop kills
//! everything it started), with no input. The lines it prints come back in batches: those that
//! arrive close together make one message, so a burst is one step, not many.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// How long a batch waits for more lines after its first.
const GATHER: Duration = Duration::from_millis(400);
/// Lines in one message at most, and characters in one line.
const LINES: usize = 40;
const LINE_CHARS: usize = 400;

/// How a watch ended.
pub enum Ended {
    Exit(String),
    Deadline,
    Stopped,
}

/// Run `command` in `dir` until it exits, `deadline` passes or `stopped` says so; give each
/// batch of lines to `lines`.
pub fn watch(
    command: &str,
    dir: &Path,
    hidden: &[&str],
    deadline: Instant,
    stopped: &dyn Fn() -> bool,
    lines: &dyn Fn(Vec<String>, usize),
) -> Result<Ended, String> {
    let mut shell = Command::new("sh");
    shell
        .arg("-c")
        .arg(command)
        .current_dir(dir)
        .stdin(Stdio::null());
    shell.stdout(Stdio::piped()).stderr(Stdio::piped());
    for name in hidden {
        shell.env_remove(name);
    }
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut shell, 0);
    let mut child = shell
        .spawn()
        .map_err(|error| format!("the command could not start: {error}"))?;
    let (sent, received) = mpsc::channel();
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
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if sent.send(line).is_err() {
                    return;
                }
            }
        });
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
    let ended = loop {
        if stopped() {
            break Ended::Stopped;
        }
        if Instant::now() >= deadline {
            break Ended::Deadline;
        }
        match received.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                since.get_or_insert_with(Instant::now);
                match batch.len() < LINES {
                    true => batch.push(line.chars().take(LINE_CHARS).collect()),
                    false => dropped += 1,
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            // Both streams closed: the command is done, or about to be.
            Err(RecvTimeoutError::Disconnected) => {
                flush(&mut batch, &mut dropped);
                let status = child.wait().map_err(|error| error.to_string())?;
                break Ended::Exit(match status.code() {
                    Some(code) => format!("exit {code}"),
                    None => "killed by a signal".into(),
                });
            }
        }
        if since.is_some_and(|at| at.elapsed() >= GATHER) {
            flush(&mut batch, &mut dropped);
            since = None;
        }
    };
    flush(&mut batch, &mut dropped);
    if !matches!(ended, Ended::Exit(_)) {
        kill(&mut child);
    }
    Ok(ended)
}

/// End the command and everything it started.
fn kill(child: &mut std::process::Child) {
    #[cfg(unix)]
    // SAFETY: a negative pid signals the process group the child leads; it is ours.
    unsafe {
        libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
#[path = "monitor_tests.rs"]
mod tests;
