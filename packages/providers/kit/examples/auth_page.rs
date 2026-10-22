//! Writes the sign-in pages to files for a visual check: `cargo run --example auth_page -- <dir>`.
fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let success = provider_kit::page::success("llx", "OpenAI");
    std::fs::write(format!("{dir}/success.html"), success).unwrap();
    let failure =
        provider_kit::page::failure("OpenAI sign-in failed.", Some("access_denied: no thanks"));
    std::fs::write(format!("{dir}/failure.html"), failure).unwrap();
}
