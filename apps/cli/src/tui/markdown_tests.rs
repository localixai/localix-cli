use super::*;
use crate::tui::style;

fn shown(text: &str, width: usize) -> String {
    render(text, width)
        .iter()
        .map(style::plain)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn blocks_lists_quotes_and_code() {
    let text = "# Done\n\nThe landing is in `site/index.html` and works on **phones** too.\n\n\
- one change in the header, which is long enough to wrap onto a second row\n\
- two\n  1. nested\n  2. again\n\n\
> a quote that also wraps when the width is small\n\n\
```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\n\
See [the docs](https://llx.sh/docs) or https://llx.sh.\n\n---\n\nEnd.";
    insta::assert_snapshot!(shown(text, 40), @r#"
    Done

    The landing is in site/index.html and
    works on phones too.

    - one change in the header, which is
      long enough to wrap onto a second row
    - two
      1. nested
      2. again

    │ a quote that also wraps when the width
    │ is small

    fn main() {
        println!("hi");
    }

    See the docs (https://llx.sh/docs) or
    https://llx.sh.

    ────────────────────────────────────────

    End.
    "#);
}

#[test]
fn tables_keep_columns_rows_and_inline_styles() {
    let text = "| | Codex | Our edit |\n|---|---|---|\n| Format | **Patch** | `exact replacement` |\n| Operations | creation | replacement |\n\nAfter the table.";
    let rows = render(text, 60);
    let screen = shown(text, 60);
    assert!(screen.contains("│"), "{screen}");
    assert!(!screen.contains("|---|"), "{screen}");
    let header = rows
        .iter()
        .position(|row| style::plain(row).contains("Codex"))
        .unwrap();
    let format = rows
        .iter()
        .position(|row| style::plain(row).contains("Format"))
        .unwrap();
    let operations = rows
        .iter()
        .position(|row| style::plain(row).contains("Operations"))
        .unwrap();
    assert!(header < format && format < operations, "{screen}");
    assert!(
        rows[format]
            .spans
            .iter()
            .any(|span| span.content == "Patch" && span.style == style::bold())
    );
    assert!(
        rows[format]
            .spans
            .iter()
            .any(|span| span.content.contains("exact") && span.style == style::code())
    );
    assert!(screen.ends_with("\n\nAfter the table."), "{screen}");
}

#[test]
fn tables_wrap_without_losing_cells_in_narrow_terminals_or_quotes() {
    let text = "> | Name | Description |\n> |:---|---:|\n> | 中文 | long words with **style** and [link](https://example.com) |\n> | empty | |";
    assert!(!shown(text, 80).contains("|:---|"));
    for width in [8, 20, 40, 80] {
        let rows = render(text, width);
        let screen = shown(text, width);
        assert!(screen.contains("中文"), "{width}: {screen}");
        assert!(screen.contains("empty"), "{width}: {screen}");
        assert!(
            rows.iter().all(|row| line_width(row) <= width),
            "{width}: {screen}"
        );
        assert!(
            rows.iter()
                .filter(|row| !row.spans.is_empty())
                .all(|row| style::plain(row).starts_with("│ ")),
            "{screen}"
        );
    }
}

#[test]
fn table_alignment_and_empty_cells_do_not_shift_columns() {
    let text = "| Left | Right | Center |\n|:---|---:|:---:|\n| x | y | z |\n| | tail | |";
    let screen = shown(text, 60);
    assert!(screen.contains("│ x    │     y │   z    │"), "{screen}");
    assert!(screen.contains("│      │  tail │        │"), "{screen}");
}

#[test]
fn styles_carry_meaning() {
    let rows = render("**bold** `code` *it*", 40);
    let spans = &rows[0].spans;
    assert_eq!(spans[0].style, style::bold());
    assert_eq!(spans[2].style, style::code());
    assert!(spans[4].style.add_modifier.contains(Modifier::ITALIC));
}

#[test]
fn nothing_is_wider_than_the_width() {
    let text =
        "a ".repeat(200) + "\n\n```\n" + &"x".repeat(100) + "\n```\n- " + &"word ".repeat(50);
    for width in [8, 20, 33] {
        for row in render(&text, width) {
            assert!(
                line_width(&row) <= width,
                "{width}: {:?}",
                style::plain(&row)
            );
        }
    }
}

#[test]
fn escapes_cannot_reach_the_terminal() {
    assert!(!shown("hi \u{1b}[2J there\n\n```\n\u{1b}]0;x\u{7}\n```", 40).contains('\u{1b}'));
}
