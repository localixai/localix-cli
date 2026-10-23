use super::*;
use crate::testing::Scratch;

/// A 3x2 PNG's first bytes: the signature and the header chunk.
fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend(3u32.to_be_bytes());
    bytes.extend(2u32.to_be_bytes());
    bytes.extend([8, 6, 0, 0, 0]);
    bytes
}

#[test]
fn pictures_are_known_by_their_first_bytes() {
    assert_eq!(kind(&png()), Some(("image/png", (3, 2))));
    let gif = b"GIF89a\x05\x00\x04\x00rest".to_vec();
    assert_eq!(kind(&gif), Some(("image/gif", (5, 4))));
    // SOI, an APP0 segment of 4 bytes, then SOF0 with height 7 and width 9.
    let jpeg = [
        0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 11, 8, 0, 7, 0, 9, 3, 0, 0, 0,
    ];
    assert_eq!(kind(&jpeg), Some(("image/jpeg", (9, 7))));
    assert_eq!(kind(b"%PDF-1.7"), None);
}

#[test]
fn a_picture_is_read_as_base64_with_its_name() {
    let dir = Scratch::new();
    let path = dir.path().join("shot one.png");
    std::fs::write(&path, png()).unwrap();
    let attached = read(&path).unwrap();
    assert_eq!(attached.name, "shot one.png");
    assert_eq!((attached.image.width, attached.image.height), (3, 2));
    assert!(attached.image.data.starts_with("iVBORw0KGgo"));
    let text = dir.path().join("notes.txt");
    std::fs::write(&text, "hi").unwrap();
    assert!(
        read(&text)
            .unwrap_err()
            .contains("not a PNG, JPEG, GIF or WebP")
    );
}

#[test]
fn a_dropped_path_is_known_however_the_terminal_writes_it() {
    let dir = Scratch::new();
    let picture = dir.path().join("shot\u{202f}one.png");
    std::fs::write(&picture, png()).unwrap();
    let notes = dir.path().join("my notes.txt");
    std::fs::write(&notes, "hi").unwrap();
    let shown = picture.display().to_string();
    assert_eq!(
        resolve(&format!("'{shown}'")),
        Some(Dropped::Image(picture.clone()))
    );
    assert_eq!(resolve(&shown), Some(Dropped::Image(picture)));
    let escaped = notes.display().to_string().replace(' ', "\\ ");
    assert_eq!(resolve(&escaped), Some(Dropped::File(notes.clone())));
    let address = format!("file://{}", notes.display()).replace(' ', "%20");
    assert_eq!(resolve(&address), Some(Dropped::File(notes)));
    assert_eq!(
        resolve(&dir.path().display().to_string()),
        Some(Dropped::File(dir.path().to_path_buf()))
    );
    assert_eq!(resolve("relative/path.png"), None);
    assert_eq!(
        resolve(&dir.path().join("missing.png").display().to_string()),
        None
    );
}

#[test]
fn a_kept_picture_is_a_copy_that_outlives_the_original() {
    let dir = Scratch::new();
    let original = dir.path().join("shot.png");
    std::fs::write(&original, png()).unwrap();
    let kept = keep(&original, &dir.path().join("pasted"));
    std::fs::remove_file(&original).unwrap();
    assert!(kept.is_file() && kept.to_string_lossy().ends_with("-shot.png"));
}
