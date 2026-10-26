use protocol::{Picture, ToolInfo};
use serde_json::json;

use super::*;

/// A directory of its own for a test, gone when it ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("llx-call-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn written(done: Done, shots: &Path) -> (Came, String) {
    let mut out = vec![];
    let came = said(done, shots, &mut out).unwrap();
    (came, String::from_utf8(out).unwrap())
}

#[test]
fn a_call_s_report_and_state_are_printed_and_its_picture_saved_to_a_file_it_names() {
    let scratch = Scratch::new();
    let shots = scratch.path().join("shots");
    let picture = Picture {
        media_type: "image/png".into(),
        data: base64::engine::general_purpose::STANDARD.encode(b"\x89PNG..."),
    };
    let done = Done::Tool {
        report: "Opened w1.".into(),
        state: Some("### Page".into()),
        image: Some(picture),
    };
    let (came, text) = written(done, &shots);
    assert_eq!(came, Came::Done);
    assert!(text.starts_with("Opened w1.\n\n### Page\n"), "{text}");
    let path = text.split("The picture: ").nth(1).unwrap();
    let path = path.split(" (read it").next().unwrap();
    assert!(path.ends_with(".png"), "{path}");
    assert_eq!(std::fs::read(path).unwrap(), b"\x89PNG...");
}

#[test]
fn how_to_call_it_is_the_tool_s_own_description_and_parameters() {
    let scratch = Scratch::new();
    let tool = ToolInfo {
        name: "window".into(),
        description: "Tabs of one window.".into(),
        parameters: json!({"type": "object", "required": ["action"]}),
    };
    let (came, text) = written(Done::Tools { tools: vec![tool] }, scratch.path());
    assert_eq!(came, Came::Done);
    assert!(text.starts_with("llx window: one call"), "{text}");
    assert!(text.contains("Tabs of one window."), "{text}");
    assert!(
        text.contains("\"required\": [\n    \"action\"\n  ]"),
        "{text}"
    );
}
