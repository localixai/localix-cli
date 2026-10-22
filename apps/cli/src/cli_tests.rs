use clap::CommandFactory;
use clap::error::ErrorKind;

use super::*;

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("lx").chain(args.iter().copied()))
}

#[test]
fn the_command_line_is_well_formed() {
    Cli::command().debug_assert();
}

#[test]
fn a_prompt_runs_with_its_options() {
    let cli = parse(&[
        "-y",
        "-C",
        "/tmp",
        "-m",
        "openai/gpt-5-mini",
        "--json",
        "-o",
        "last.txt",
        "--max-turns",
        "7",
        "--show-browser",
        "fix the tests",
    ])
    .unwrap();
    assert_eq!(cli.prompt.as_deref(), Some("fix the tests"));
    assert!(cli.command.is_none());
    let run = cli.run;
    assert!(run.yes && run.json && run.show_browser);
    assert_eq!(run.cd, Some(PathBuf::from("/tmp")));
    assert_eq!(run.models.model, "openai/gpt-5-mini");
    assert_eq!(run.output_last_message, Some(PathBuf::from("last.txt")));
    assert_eq!(run.max_turns, 7);
    assert_eq!(run.max_steps, 40);
    assert_eq!(run.timeout, 1800);
}

#[test]
fn no_prompt_is_the_prompt_line() {
    let cli = parse(&[]).unwrap();
    assert!(cli.prompt.is_none() && cli.command.is_none());
    assert_eq!(parse(&["-"]).unwrap().prompt.as_deref(), Some("-"));
}

#[test]
fn subcommands_parse() {
    assert!(matches!(
        parse(&["login"]).unwrap().command,
        Some(Command::Login(LoginArgs {
            with_token: false,
            action: None
        }))
    ));
    assert!(matches!(
        parse(&["login", "--with-token"]).unwrap().command,
        Some(Command::Login(LoginArgs {
            with_token: true,
            ..
        }))
    ));
    assert!(matches!(
        parse(&["login", "status"]).unwrap().command,
        Some(Command::Login(LoginArgs {
            action: Some(LoginAction::Status),
            ..
        }))
    ));
    assert!(matches!(
        parse(&["logout"]).unwrap().command,
        Some(Command::Logout)
    ));
    assert!(matches!(
        parse(&["sessions"]).unwrap().command,
        Some(Command::Sessions)
    ));
    let Some(Command::Doctor(models)) = parse(&["doctor", "-m", "x/y"]).unwrap().command else {
        panic!("doctor")
    };
    assert_eq!(models.model, "x/y");
}

#[test]
fn resume_takes_an_id_or_the_last_session() {
    let resume = |args: &[&str]| match parse(args).unwrap().command {
        Some(Command::Resume(resume)) => resume,
        _ => panic!("not resume"),
    };
    let by_id = resume(&["resume", "1700000000-ab12", "go on", "-y"]);
    assert_eq!(by_id.target(), Ok((Some("1700000000-ab12"), Some("go on"))));
    assert!(by_id.run.yes);
    let last = resume(&["resume", "--last", "now the README"]);
    assert_eq!(last.target(), Ok((None, Some("now the README"))));
    assert_eq!(resume(&["resume", "--last"]).target(), Ok((None, None)));
    assert!(resume(&["resume", "--last", "a", "b"]).target().is_err());
    let missing = parse(&["resume"]).unwrap_err();
    assert_eq!(missing.kind(), ErrorKind::MissingRequiredArgument);
}

#[test]
fn removed_and_unknown_flags_are_usage_errors() {
    for flag in [
        "--expect",
        "--no-llm",
        "--dry-run",
        "--keep-open",
        "--headless",
        "-c",
    ] {
        let error = parse(&[flag, "x"]).unwrap_err();
        assert!(error.use_stderr(), "{flag}");
    }
    let zero = parse(&["--max-turns", "0", "x"]).unwrap_err();
    assert_eq!(zero.kind(), ErrorKind::ValueValidation);
    assert!(zero.to_string().contains("1..=10000"), "{zero}");
    assert!(!zero.to_string().contains("18446744073709551615"));
    assert!(parse(&["--timeout", "86401", "x"]).is_err());
    assert!(parse(&["--timeout", "86400", "x"]).is_ok());
}

#[test]
fn a_prompt_conflicts_with_a_subcommand() {
    assert!(parse(&["fix it", "sessions"]).is_err());
    assert!(parse(&["login", "--with-token", "status"]).is_err());
}

#[test]
fn help_and_version_are_not_errors() {
    for flag in ["--help", "-h", "--version", "-V"] {
        let shown = parse(&[flag]).unwrap_err();
        assert!(!shown.use_stderr(), "{flag}");
    }
    let help = Cli::command().render_long_help().to_string();
    for heading in [
        "Model:",
        "Workspace:",
        "Approvals:",
        "Output:",
        "Limits:",
        "Browser:",
    ] {
        assert!(help.contains(heading), "no {heading} in --help");
    }
    assert!(help.contains("Examples:"));
}
