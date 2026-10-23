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
