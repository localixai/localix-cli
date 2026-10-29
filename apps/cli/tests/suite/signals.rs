//! What a signal does to a run at no terminal (`exec`): SIGHUP and SIGTERM stop the task as
//! Ctrl-C does, and a second signal quits the process without leaving what its tools started
//! behind.

use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::common::home::Home;
use crate::common::model::{Model, Reply};

#[expect(
    clippy::disallowed_methods,
    reason = "a process has no event to wait on in std: it is looked at every few ms, bounded by a deadline"
)]
fn backoff(millis: u64) {
    std::thread::sleep(Duration::from_millis(millis));
}

fn start(home: &Home, model: &Model) -> Child {
    let mut command = home.with_model(model, &["exec", "-y", "--timeout", "120", "x"]);
    let lead = || {
        // SAFETY: setsid takes nothing and only makes the child a session's leader.
        unsafe { libc::setsid() };
        Ok(())
    };
    // SAFETY: between fork and exec the closure only calls setsid, which is async-signal-safe.
    unsafe { std::os::unix::process::CommandExt::pre_exec(&mut command, lead) };
    command
        .current_dir(home.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// The pid the command wrote to `file`, once it has.
fn pid_in(file: &Path) -> i32 {
    let end = Instant::now() + Duration::from_secs(20);
    while Instant::now() < end {
        if let Ok(text) = std::fs::read_to_string(file)
            && let Ok(pid) = text.trim().parse()
        {
            return pid;
        }
        backoff(20);
    }
    panic!("the command never wrote its pid");
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only checks that the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn gone_soon(pid: i32) -> bool {
    let end = Instant::now() + Duration::from_secs(10);
    while alive(pid) && Instant::now() < end {
        backoff(20);
    }
    !alive(pid)
}

/// How the process ended, as a shell reads it (128 and the signal for a death by one).
fn exit_of(child: &mut Child) -> Option<i32> {
    let end = Instant::now() + Duration::from_secs(15);
    while Instant::now() < end {
        if let Some(status) = child.try_wait().unwrap() {
            let signal = std::os::unix::process::ExitStatusExt::signal(&status);
            return Some(signal.map_or_else(|| status.code().unwrap(), |signal| 128 + signal));
        }
        backoff(20);
    }
    None
}

fn signal(child: &Child, signal: libc::c_int) {
    // SAFETY: kill only sends a signal to a process the test started.
    unsafe { libc::kill(child.id().cast_signed(), signal) };
}

#[test]
fn a_hangup_stops_the_task_and_the_command_it_runs() {
    let home = Home::new();
    let file = home.path().join("pid");
    let command = format!("echo $$ > {}; exec sleep 300", file.display());
    let model = Model::start(vec![Reply::call("bash", json!({ "command": command }))]);
    let mut child = start(&home, &model);
    let pid = pid_in(&file);
    signal(&child, libc::SIGHUP);
    assert_eq!(
        exit_of(&mut child),
        Some(130),
        "a hangup stops, it does not kill"
    );
    assert!(gone_soon(pid), "the command outlived the run");
}

#[test]
fn a_second_ctrl_c_quits_and_takes_the_monitors_with_it() {
    let home = Home::new();
    let file = home.path().join("pid");
    let command = format!("echo $$ > {}; exec sleep 300", file.display());
    let model = Model::start(vec![
        Reply::call(
            "jobs",
            json!({"action": "monitor", "name": "m", "command": command}),
        ),
        // The model never answers: the task cannot end by itself.
        Reply::After(home.path().join("never"), Box::new(Reply::text("late"))),
    ]);
    let mut child = start(&home, &model);
    let pid = pid_in(&file);
    signal(&child, libc::SIGINT);
    backoff(300);
    signal(&child, libc::SIGINT);
    assert_eq!(exit_of(&mut child), Some(130));
    assert!(gone_soon(pid), "the monitor outlived the run");
}
