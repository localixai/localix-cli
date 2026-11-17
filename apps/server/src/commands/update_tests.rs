//! The update notice does not wait for the network, and only a newer stable tag can install.

use std::sync::mpsc::sync_channel;

use super::*;

fn release(tag: &str) -> Reply {
    Reply {
        status: 200,
        body: serde_json::json!({"tag_name": tag, "draft": false, "prerelease": false}).to_string(),
        ..Reply::default()
    }
}

#[test]
fn only_a_newer_stable_release_is_offered() {
    assert_eq!(
        newer("0.6.2", &release("v0.6.10")),
        Ok(Some("0.6.10".into()))
    );
    assert_eq!(newer("0.6.2", &release("v0.6.2")), Ok(None));
    assert_eq!(newer("0.6.2", &release("v0.6.1")), Ok(None));
    assert_eq!(
        newer("0.6.2-rc.1", &release("v0.6.2")),
        Ok(Some("0.6.2".into()))
    );
    assert!(newer("0.6.2", &release("v0.6.3-rc.1")).is_err());
    assert!(newer("0.6.2", &release("v0.6.3\nhello")).is_err());
    assert!(
        newer(
            "0.6.2",
            &Reply {
                status: 403,
                ..release("v0.7.0")
            }
        )
        .is_err()
    );
}

#[test]
fn a_notice_arrives_after_the_check_returns_without_waiting_for_the_fetch() {
    let (go, blocked) = sync_channel(0);
    let (sent, notice) = sync_channel(1);
    let worker = background(
        move || {
            blocked.recv().unwrap();
            newer("0.6.2", &release("v0.7.0"))
        },
        move |tag| sent.send(tag).unwrap(),
    )
    .unwrap();
    assert!(notice.try_recv().is_err());
    go.send(()).unwrap();
    assert_eq!(
        notice.recv_timeout(Duration::from_secs(2)).unwrap(),
        "0.7.0"
    );
    worker.join().unwrap();
}

#[test]
fn a_failed_background_check_says_nothing() {
    let (sent, notice) = sync_channel(1);
    background(|| Err("offline".into()), move |tag| sent.send(tag).unwrap())
        .unwrap()
        .join()
        .unwrap();
    assert!(notice.try_recv().is_err());
}

#[test]
fn the_update_directory_must_hold_the_running_binary() {
    let scratch = crate::test_support::Scratch::new();
    let dir = scratch.path().to_path_buf();
    let binary = dir.join("llx");
    let other = dir.join("other");
    std::fs::write(&other, "").unwrap();
    std::os::unix::fs::symlink(&other, &binary).unwrap();
    assert_eq!(directory(&other, &dir), None);
    assert_eq!(directory(&binary, &dir), None);
    std::fs::remove_file(&binary).unwrap();
    std::fs::write(&binary, "").unwrap();
    assert_eq!(directory(&binary, &dir), Some(dir));
}
