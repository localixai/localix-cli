//! Opens a page and prints scripts' results: a probe for building the snapshot. `cargo run --example inspect -- <url> <script>...` (each script runs in turn).
#![expect(
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::disallowed_methods,
    reason = "a developer's probe: it prints what it finds, and a missing argument ends it"
)]

use std::time::Duration;

use tools::browser::{self, Driver, Window};

fn main() {
    let mut args = std::env::args().skip(1);
    let url = args.next().expect("url");
    let scripts: Vec<String> = args.collect();
    let window = Window {
        url,
        title: "inspect".into(),
        headless: true,
        viewport: browser::Viewport::DESKTOP,
    };
    browser::host(
        || std::process::exit(130),
        move |browser| {
            let driver = match browser.open(window) {
                Ok(driver) => driver,
                Err(error) => {
                    eprintln!("{error}");
                    return 3;
                }
            };
            let _ = driver.wait_loaded(Duration::from_secs(20));
            for script in &scripts {
                match driver.evaluate(script) {
                    Ok(result) => println!("{result}"),
                    Err(error) => println!("error: {error}"),
                }
                std::thread::sleep(Duration::from_millis(800));
            }
            0
        },
    );
}
