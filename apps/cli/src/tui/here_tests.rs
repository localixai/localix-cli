use std::path::Path;
use std::time::Instant;

use protocol::Event;

use super::shown;
use crate::tui::app::App;
use crate::tui::mention::Files;
use crate::tui::test_support::{call, done, history, label, ran, result};

#[test]
fn paths_in_the_project_read_from_it_and_its_cd_is_dropped() {
    let root = Some(Path::new("/work/stats"));
    let edited = label("Editing", "Edited", "/work/stats/src/stats.py");
    assert_eq!(shown(edited, root).subject, "src/stats.py");
    let command = ran("cd /work/stats && python3 -m unittest /work/stats/test_stats.py");
    assert_eq!(
        shown(command, root).subject,
        "python3 -m unittest test_stats.py"
    );
    // Another project's paths, and one that only starts like this one's, stay whole.
    let other = ran("cat /work/statsx/a /work/other/b");
    assert_eq!(shown(other.clone(), root).subject, other.subject);
    assert_eq!(shown(ran("ls /work/stats"), root).subject, "ls .");
    assert_eq!(
        shown(ran("ls /work/stats/x"), None).subject,
        "ls /work/stats/x"
    );
}

#[test]
fn the_transcript_shows_a_finished_edit_by_its_path_in_the_project() {
    let now = Instant::now();
    let mut app = App::new(60, 20, vec![]);
    app.files = Files::new("/work/stats".into(), Box::new(drop));
    app.event(Event::TaskStarted("fix it".into()), now);
    let edited = label("Patching", "Patched", "/work/stats/stats.py");
    app.event(call("1", "apply_patch", &edited), now);
    let report = done("Updated stats.py (+1 -1 lines)");
    app.event(result("1", "apply_patch", &edited, report), now);
    let shown = history(&mut app);
    assert!(shown.contains("Patched stats.py"), "{shown}");
    assert!(!shown.contains("/work/stats"), "{shown}");
}
