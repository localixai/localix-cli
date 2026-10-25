use super::*;

fn rules(allow: &[&str], deny: &[&str]) -> Rules {
    let owned = |list: &[&str]| list.iter().map(ToString::to_string).collect::<Vec<_>>();
    Rules::new(&owned(allow), &owned(deny)).unwrap()
}

fn allowed(rule: &str, command: &str) -> bool {
    rules(&[rule], &[]).judge("bash", command) == Ruling::Allowed
}

fn denied(rule: &str, command: &str) -> bool {
    matches!(
        rules(&[], &[rule]).judge("bash", command),
        Ruling::Denied(_)
    )
}

#[test]
fn a_wildcard_stands_for_any_text_and_the_words_before_it_limit_the_rule() {
    // Claude Code's table of what a rule matches and what it does not.
    for (rule, yes, no) in [
        (
            "bash(npm run build)",
            &["npm run build"][..],
            &["npm run build --watch"][..],
        ),
        (
            "bash(npm run *)",
            &["npm run build", "npm run test --watch", "npm run"],
            &["npm install"],
        ),
        (
            "bash(git log * main)",
            &["git log --oneline main", "git log -5 main"],
            &["git log main", "git push origin main"],
        ),
        ("bash(* --version)", &["node --version"], &["node -v"]),
        ("bash(ls *)", &["ls -la", "ls"], &["lsof"]),
        ("bash(ls*)", &["ls -la", "lsof"], &[]),
        ("bash(ls:*)", &["ls -la", "ls"], &["lsof"]),
    ] {
        for command in yes {
            assert!(allowed(rule, command), "{rule} should allow {command}");
        }
        for command in no {
            assert!(!allowed(rule, command), "{rule} should not allow {command}");
        }
    }
}

#[test]
fn every_part_of_a_chain_must_be_allowed() {
    let both = rules(&["bash(git status)", "bash(cargo test *)"], &[]);
    assert_eq!(
        both.judge("bash", "git status && cargo test -q"),
        Ruling::Allowed
    );
    assert_eq!(
        both.judge("bash", "cargo test | tail -5"),
        Ruling::Unsettled
    );
    for chained in [
        "cargo test && rm -rf x",
        "cargo test || rm -rf x",
        "cargo test; rm -rf x",
        "cargo test | sh",
        "cargo test & rm -rf x",
        "cargo test\nrm -rf x",
    ] {
        assert_eq!(
            rules(&["bash(cargo test *)"], &[]).judge("bash", chained),
            Ruling::Unsettled,
            "{chained}"
        );
    }
}

#[test]
fn a_separator_inside_quotes_is_text() {
    assert!(allowed("bash(echo *)", "echo 'a && b; c | d'"));
    assert!(allowed("bash(echo *)", "echo \"a && b\""));
}

#[test]
fn what_cannot_be_read_for_sure_is_never_allowed() {
    for command in [
        "echo $(rm -rf x)",
        "echo `rm -rf x`",
        "echo <(rm -rf x)",
        "echo 'unclosed",
        "echo hi &&",
        "echo hi > out.txt",
        "echo hi >> out.txt",
        "echo hi | tee out.txt",
        "eval rm -rf x",
        "sh -c 'rm -rf x'",
        "bash -c 'rm -rf x'",
        "cat <<EOF",
    ] {
        assert!(!allowed("bash(echo *)", command), "{command}");
    }
    assert!(allowed("bash(echo *)", "echo hi > /dev/null"));
    assert!(allowed("bash(echo *)", "echo hi 2>&1"));
}

#[test]
fn wrappers_are_looked_through_but_an_assignment_stops_an_allow() {
    assert!(allowed("bash(npm test *)", "timeout 30 npm test"));
    assert!(allowed("bash(npm test *)", "nice -n 5 npm test"));
    assert!(allowed("bash(grep *)", "xargs grep x"));
    assert!(!allowed("bash(grep *)", "xargs -n1 grep x"));
    assert!(!allowed("bash(npm test *)", "LD_PRELOAD=x.so npm test"));
}

#[test]
fn a_deny_matches_any_part_past_any_assignment() {
    assert!(denied("bash(rm *)", "FOO=bar rm -rf tmp"));
    assert!(denied("bash(rm *)", "echo hi && rm -rf tmp"));
    assert!(denied("bash(rm *)", "echo $(rm -rf tmp)"));
    assert!(denied("bash(git push *)", "timeout 9 git push origin main"));
    assert!(!denied("bash(rm *)", "echo 'rm -rf tmp'"));
}

#[test]
fn deny_comes_before_allow() {
    let both = rules(&["bash(git *)"], &["bash(git push *)"]);
    assert_eq!(both.judge("bash", "git status"), Ruling::Allowed);
    assert!(
        matches!(both.judge("bash", "git push"), Ruling::Denied(why) if why.contains("bash(git push *)"))
    );
}

#[test]
fn a_bare_tool_name_matches_every_call_of_it_and_only_of_it() {
    let any = rules(&["bash", "browser"], &[]);
    assert_eq!(any.judge("bash", "rm -rf $(pwd)"), Ruling::Allowed);
    assert_eq!(any.judge("browser", ""), Ruling::Allowed);
    assert_eq!(any.judge("edit", "a.md"), Ruling::Unsettled);
    assert_eq!(
        rules(&["Bash(echo *)"], &[]).judge("bash", "echo hi"),
        Ruling::Allowed
    );
}

#[test]
fn a_pattern_needs_a_subject_to_match() {
    // Typing into a terminal session carries no command a pattern could judge.
    assert_eq!(
        rules(&["bash(echo *)"], &[]).judge("bash", ""),
        Ruling::Unsettled
    );
    // `bash(*)` is the bare name, as in Claude Code: every call.
    assert!(matches!(
        rules(&[], &["bash(*)"]).judge("bash", ""),
        Ruling::Denied(_)
    ));
    assert_eq!(
        rules(&["bash(*)"], &[]).judge("bash", "echo $(date)"),
        Ruling::Allowed
    );
}

#[test]
fn an_edit_rule_is_a_glob_from_the_workspace_root() {
    for (rule, yes, no) in [
        (
            "edit(docs/**)",
            &["docs/a.md", "docs/x/y.md"][..],
            &["src/docs/a.md", "a.md"][..],
        ),
        ("edit(*.md)", &["a.md", "docs/a.md"], &["a.rs"]),
        ("edit(src/*.rs)", &["src/a.rs"], &["src/x/a.rs"]),
        (
            "edit(CHANGELOG.md)",
            &["CHANGELOG.md"],
            &["docs/CHANGELOG.md.bak"],
        ),
    ] {
        let judged = rules(&[rule], &[]);
        for path in yes {
            assert_eq!(judged.judge("edit", path), Ruling::Allowed, "{rule} {path}");
        }
        for path in no {
            assert_eq!(
                judged.judge("edit", path),
                Ruling::Unsettled,
                "{rule} {path}"
            );
        }
    }
    assert_eq!(
        rules(&["edit(docs/**)"], &[]).judge("edit", "docs/../src/a.rs"),
        Ruling::Unsettled
    );
}

#[test]
fn a_rule_that_cannot_be_read_says_which() {
    for rule in [
        "bash(echo *",
        "nosuchtool",
        "edit(//etc/**)",
        "edit(~/x)",
        "edit(../x)",
        "bash()",
        "",
    ] {
        let error = Rules::new(&[rule.to_string()], &[]).unwrap_err();
        assert!(error.contains(&format!("`{rule}`")), "{rule}: {error}");
    }
}

#[test]
fn the_rules_read_back_for_the_model() {
    assert_eq!(rules(&[], &[]).describe(), None);
    let text = rules(&["bash(cargo test *)"], &["bash(git push *)"])
        .describe()
        .unwrap();
    assert!(
        text.contains("bash(cargo test *)") && text.contains("bash(git push *)"),
        "{text}"
    );
}
