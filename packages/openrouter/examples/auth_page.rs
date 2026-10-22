//! Writes the sign-in pages to files for a visual check: `cargo run --example auth_page -- <dir>`.
fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::write(
        format!("{dir}/success.html"),
        openrouter::auth_page::success("llx"),
    )
    .unwrap();
    let failure = openrouter::auth_page::failure(
        "OpenRouter sign-in failed.",
        Some("OpenRouter answered 400: invalid code"),
    );
    std::fs::write(format!("{dir}/failure.html"), failure).unwrap();
}
