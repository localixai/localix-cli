use super::*;
use crate::testing::Scratch;

#[test]
fn a_write_creates_the_parents_and_leaves_no_temporary_file() {
    let dir = Scratch::new();
    let path = dir.path().join("a/b/c.txt");
    write(&path, b"hello").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
    let names: Vec<_> = fs::read_dir(path.parent().unwrap())
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name())
        .collect();
    assert_eq!(names, ["c.txt"]);
}

#[test]
fn a_write_replaces_the_content() {
    let dir = Scratch::new();
    let path = dir.file("x.txt", "old content");
    write(&path, b"new").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
}

#[cfg(unix)]
#[test]
fn an_executable_stays_executable() {
    use std::os::unix::fs::PermissionsExt;
    let dir = Scratch::new();
    let path = dir.file("run.sh", "#!/bin/sh\n");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    write(&path, b"#!/bin/sh\necho hi\n").unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o755
    );
}

#[test]
fn temporary_names_differ_and_stay_beside_the_file() {
    let path = Path::new("/w/src/lib.rs");
    let (a, b) = (temporary(path), temporary(path));
    assert_ne!(a, b);
    assert_eq!(a.parent(), path.parent());
    assert!(
        a.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".lib.rs.")
    );
}

#[test]
fn a_read_only_file_is_refused_and_kept() {
    let dir = Scratch::new();
    let path = dir.file("locked.txt", "vendored");
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions.clone()).unwrap();
    let error = write(&path, b"changed").unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(error.to_string(), "it is read-only");
    assert_eq!(fs::read_to_string(&path).unwrap(), "vendored");
    // Removable again, so the scratch directory goes.
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(&path, permissions).unwrap();
}

#[cfg(unix)]
#[test]
fn every_hard_link_sees_the_new_content() {
    let dir = Scratch::new();
    let path = dir.file("a.txt", "old");
    let other = dir.path().join("b.txt");
    fs::hard_link(&path, &other).unwrap();
    write(&path, b"new").unwrap();
    assert_eq!(fs::read_to_string(&other).unwrap(), "new");
}

#[test]
fn a_long_name_still_gets_a_temporary_beside_it() {
    let dir = Scratch::new();
    let path = dir.path().join(format!("{}.txt", "n".repeat(240)));
    write(&path, b"fits").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "fits");
    let name = temporary(&path);
    assert!(name.file_name().unwrap().len() < 140, "{}", name.display());
}

#[cfg(unix)]
#[test]
fn a_symlink_planted_at_a_temporary_name_is_never_written_through() {
    let dir = Scratch::new();
    let outside = Scratch::new();
    let victim = outside.file("victim.txt", "untouched");
    let path = dir.file("x.txt", "old");
    // The names the next writes would take: each one is a symlink out of the directory.
    let taken = temporary(&path);
    let taken = taken.file_name().unwrap().to_string_lossy().into_owned();
    let (stem, number) = taken.trim_end_matches(".tmp").rsplit_once('-').unwrap();
    let number: usize = number.parse().unwrap();
    for next in number + 1..number + 6 {
        let planted = dir.path().join(format!("{stem}-{next}.tmp"));
        std::os::unix::fs::symlink(&victim, planted).unwrap();
    }
    write(&path, b"new").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    assert_eq!(fs::read_to_string(&victim).unwrap(), "untouched");
}
