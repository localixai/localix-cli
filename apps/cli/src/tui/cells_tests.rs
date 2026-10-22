use super::*;
use crate::tui::style;

fn shown(lines: Vec<Line>) -> String {
    lines
        .iter()
        .map(style::plain)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_task_from_question_to_end() {
    let width = 60;
    let mut out = vec![];
    out.extend(header("0.1.0", &["haiku-5.5", "~/shop", ""], width));
    out.push(Line::default());
    out.extend(user(
        "make a landing page for a phone shop, with prices and a contact form",
        width,
    ));
    out.push(Line::default());
    out.extend(agent(
        "The page is in `site/index.html`:\n\n- a hero with **three** phones\n- a contact form",
        width,
    ));
    out.push(Line::default());
    out.extend(end(
        &End {
            status: "achieved",
            millis: 48_200,
            turns: 6,
            cost: 0.0087,
        },
        width,
    ));
    out.extend(error(
        "the model is unreachable: timed out after 90s",
        width,
    ));
    out.extend(note("resumed session 2026-10-23-1", width));
    insta::assert_snapshot!(shown(out), @r"
    llx 0.1.0 · haiku-5.5 · ~/shop

    ❯ make a landing page for a phone shop, with prices and a
      contact form

    • The page is in site/index.html:

      - a hero with three phones
      - a contact form

    ─ Worked for 48s · 6 turns · $0.0087 ───────────────────────
    ■ the model is unreachable: timed out after 90s
      resumed session 2026-10-23-1
    ");
}

#[test]
fn the_end_rule_fills_the_width_and_says_how_it_ended() {
    let rule = end(
        &End {
            status: "interrupted",
            millis: 65_000,
            turns: 1,
            cost: 0.0,
        },
        50,
    );
    assert_eq!(
        style::plain(&rule[0]),
        format!(
            "─ Stopped after 1m 05s · 1 turn · $0.0000 {}",
            "─".repeat(8)
        )
    );
    assert_eq!(super::super::wrap::line_width(&rule[0]), 50);
}

#[test]
fn durations_read_naturally() {
    assert_eq!(duration(2_140), "2.1s");
    assert_eq!(duration(48_900), "48s");
    assert_eq!(duration(65_000), "1m 05s");
    assert_eq!(duration(3_720_000), "1h 02m");
}

#[test]
fn many_lines_from_the_person_stay_lines() {
    assert_eq!(shown(user("one\ntwo", 40)), "❯ one\n  two");
}

#[test]
fn a_note_keeps_its_rows_and_columns() {
    assert_eq!(
        shown(note("model      haiku\nworkspace  ~/shop\n", 40)),
        "  model      haiku\n  workspace  ~/shop"
    );
}

#[test]
fn the_header_stays_one_row_and_cuts_the_folder_from_the_start() {
    let rows = header(
        "0.1.0",
        &[
            "haiku-5.5",
            "/private/tmp/a/very/long/path/to/the/workspace",
        ],
        40,
    );
    assert_eq!(shown(rows), "llx 0.1.0 · haiku-5.5 · …o/the/workspace");
}
