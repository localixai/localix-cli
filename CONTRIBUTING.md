# Contributing

Thanks for helping. llx is meant to stay small and fast: seven tools (`read`, `edit`, `bash`,
`window`, `ask`, `agents`, `jobs`), synchronous code, no async runtime, and a release binary under
4 MB. A change that adds a dependency has to earn its size.

## Before you start

Read [ARCHITECTURE.md](ARCHITECTURE.md) and [AGENTS.md](AGENTS.md). ARCHITECTURE.md is binding:
it fixes the layers, what may know what, and every rule by id (L, C, P, A, K, W, R, S).
AGENTS.md says what each file is, with the behaviour and code rules. They apply to people as
much as to coding agents, and the tests in `apps/cli/tests/suite/repo/` check the layering and
the size budgets. A change that does not fit a rule is redesigned, or the rule is changed first,
in its own commit.

For a larger change, open an issue first to agree on the shape.

## Setup

The toolchain is the latest stable Rust (`rust-toolchain.toml` installs it with rustfmt and
clippy); the code also builds on the workspace's `rust-version`. Linux needs WebKitGTK and, for
the browser tests, a display:

```bash
sudo apt-get install libwebkit2gtk-4.1-dev xvfb   # Debian, Ubuntu
```

## Checks

Run these before you push; CI runs the same on macOS and Linux (Windows is not supported for now):

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace          # on a Linux server: xvfb-run -a cargo test --workspace
```

CI also checks the minimum Rust version, the line coverage (`scripts/coverage.sh`), and fails when
the Linux release binary passes 5 MiB (`cargo build --release && ls -la target/release/llx`).
The code stays under 50,000 lines of working code and 300 per file (ARCHITECTURE.md S1, S2).

For changes to the loop, the page probe or the instructions, run the live evals too
(`python3 evals/run.py --repeat 3`; paid, a few cents) and put the result in the pull request.

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
