use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("llx-store-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_saved_secret_is_readable_by_the_person_alone() {
    let dir = scratch("save");
    let path = dir.join("nested").join("secret");
    save(&path, "  token \n").unwrap();
    assert_eq!(read(&path).as_deref(), Some("token"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    assert!(remove(&path).unwrap());
    assert!(!remove(&path).unwrap(), "already gone");
    assert_eq!(read(&path), None);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn a_file_saved_with_wider_permissions_is_narrowed() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch("narrow");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("secret");
    std::fs::write(&path, "old").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    save(&path, "new").unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    std::fs::remove_dir_all(&dir).unwrap();
}
