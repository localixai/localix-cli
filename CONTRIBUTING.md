# Contributing

Thanks for helping. lx is meant to stay small and fast: a few tools (read, edit, bash, the
browser), synchronous code, no async runtime, and a release binary of about 1.6 MB. A change that
adds a dependency has to earn its size.

## Before you start

Read [AGENTS.md](AGENTS.md): it holds the layer rules (L1-L5: dependencies point inward, and the
agent and the hands name no tool or provider), the behaviour rules, and the code rules. They
apply to people as much as to coding agents, and `apps/cli/tests/architecture.rs` checks the
layering.

For a larger change, open an issue first to agree on the shape.

## Setup

The toolchain is the latest stable Rust (`rust-toolchain.toml` installs it with rustfmt and
clippy); the code also builds on the workspace's `rust-version`. Linux needs WebKitGTK and, for
the browser tests, a display:

```bash
sudo apt-get install libwebkit2gtk-4.1-dev xvfb   # Debian, Ubuntu
```

## Checks

Run these before you push; CI runs the same on macOS, Linux and Windows:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # on a Linux server: xvfb-run -a cargo test --workspace
```

CI also checks the minimum Rust version, and fails when the Linux release binary passes 2.5 MiB
(`cargo build --release && ls -la target/release/lx`).

For changes to the loop, the page probe or the instructions, run the live evals too
(`python3 evals/run.py --repeat 3`; paid, about $0.03) and put the result in the pull request.

## Code

- Small, focused modules; new files stay under 300 lines; rustfmt defaults.
- Doc comments and comments explain why, not what.
- Unit tests never live inline: `x.rs` ends with

  ```rust
  #[cfg(test)]
  #[path = "x_tests.rs"]
  mod tests;
  ```

  and the tests are in `x_tests.rs` next to it. The architecture test enforces it.
- Tests use fake ports, never the network. A driver or probe fix comes with a fixture under
  `packages/tools/tests/fixtures/` and an assertion in `packages/tools/tests/browser.rs` that
  fails without the fix.
- Exit codes and the `--json` schema are a public contract: change them only with a schema bump
  and a CHANGELOG entry.
- When behaviour changes, update together the `--help` text (the clap doc comments in
  `apps/cli/src/cli.rs`), `README.md`, `CHANGELOG.md` under `## Unreleased`, and
  `evals/README.md` when tasks change.

## Pull requests

Keep one change per pull request, with a description of what and why. Releases are cut by the
maintainers ([RELEASING.md](RELEASING.md)). By contributing you agree that your work is licensed
under the MIT license of this repository.
