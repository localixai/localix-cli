## What and why

<!-- What changes, and the reason. Link the issue if there is one. -->

## Checks

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass
- [ ] Layer rules respected (AGENTS.md); new unit tests live in `x_tests.rs` files
- [ ] Behaviour changes: `--help` (apps/cli/src/cli.rs), README.md, CHANGELOG.md (Unreleased), and evals/README.md when tasks change, updated together
- [ ] Loop, probe or prompt changes: `python3 evals/run.py --repeat 3` run, results below
