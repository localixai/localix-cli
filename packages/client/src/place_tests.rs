use super::*;

#[test]
fn a_place_is_where_the_process_is_with_its_environment_and_the_given_command_line() {
    let theme = Theme {
        fg: [1, 2, 3],
        bg: [4, 5, 6],
        cursor: [7, 8, 9],
    };
    let place = here(
        Path::new("/work/project"),
        Some(theme),
        vec!["window".into()],
    );
    assert_eq!(place.cwd, "/work/project");
    assert_eq!(place.theme, Some(theme));
    assert_eq!(place.args, ["window"]);
    let dir = std::env::current_dir().unwrap();
    assert_eq!(place.dir, dir.display().to_string());
    let path = std::env::var("PATH").unwrap();
    assert!(place.env.contains(&("PATH".to_string(), path)));
}
