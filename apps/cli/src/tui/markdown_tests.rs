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
