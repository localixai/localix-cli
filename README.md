<p align="center">
  <img src="site/public/logo.svg" width="72" height="72" alt="llx">
</p>

<h1 align="center">llx</h1>

<p align="center">
  <strong>An ultrafast, lightweight coding agent for the terminal</strong>
</p>

<p align="center">
  One binary under 4 MB that starts in about 5 ms. Seven tools, no plugins.
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="docs/usage.md">Documentation</a> ·
  <a href="docs/mcp.md">Use from another agent</a> ·
  <a href="https://llx.sh">llx.sh</a>
</p>

```text
$ llx exec -y "the tests in tests/parse.rs fail: fix the parser, not the tests"
   1.1s ▸ bash cargo test --test parse
   3.4s ▸ read src/parse.rs
   5.0s ▸ edit src/parse.rs
   6.2s ▸ bash cargo test --test parse
achieved in 9.0s · 5 turns · $0.0058
parse_range treated an empty upper bound as 0; it now means "to the end". All 14 tests pass.
```

<sub>An example: times, tokens and cost vary. The trace goes to stderr and the answer alone to stdout.</sub>

## Install

```bash
npm install -g @rsaryev/llx
```

Or with the script (macOS and Linux x86_64), which checks the release against `SHA256SUMS` and installs `llx` into `~/.local/bin` (`LLX_VERSION=0.2.0` picks a version, `LLX_INSTALL_DIR` the place):

```bash
curl -fsSL https://raw.githubusercontent.com/rsaryev/llx/main/scripts/install.sh | sh
```

From a clone: `cargo install --locked --path apps/cli` (Rust 1.88 or newer). Windows is not supported for now.

The browser is the system's own WebView: WKWebView on macOS; on Linux, WebKitGTK (`libwebkit2gtk-4.1-0`) and a display.

## Quick start

```bash
cd your-project
llx              # a session: type /login first, then a task
llx doctor       # checks the sign-ins, both models, the WebView and the shell
```

`/login` signs you in to a provider and picks a model:

- **OpenAI** with a ChatGPT sign-in: your browser opens and comes back to llx by itself; requests are billed to your ChatGPT plan.
- **OpenRouter** with a key: paste it, or sign in in the browser. `OPENROUTER_API_KEY` works too.
- **Any provider of the [model catalog](MODELS.md)** (Anthropic, Google, xAI, DeepSeek, Mistral, Groq… about 200) with a key: paste it, or set the variable models.dev names for it (`ANTHROPIC_API_KEY`…).

```bash
llx "fix the failing test in src/parse.rs"          # a session that starts with this task
llx -c                                              # go on with the newest session here
llx --mode plan "how should we split src/parse.rs?" # investigate and plan; change no file
git diff | llx -p "review this change"              # one task from stdin, then exit
llx exec --json -y "update the changelog"           # JSON events, one per line
```

## What it has

| Tool | What it does |
| --- | --- |
| `read`, `edit` | Files: numbered lines, exact and atomic replacement |
| `bash` | Commands that run to their end, killed with everything they started |
| `window` | Everything it opens, as tabs of one window: real web pages and terminal sessions, acted in with a short `goal` or the kind's own fields |
| `ask` | A question to you, when a decision is yours |
| `agents` | Other agents in parallel, each a session you can switch to |
| `jobs` | Monitors (a background command whose lines come back) and schedules |

No plugins, no MCP servers consumed, no extra tools: a model with a shell and a browser needs little more. Details in [docs/tools.md](docs/tools.md).

## Safe by default

Reading never asks. Every command, file change and page action asks first, in one place, and a mode decides what happens: `default` asks, `plan` investigates and refuses file changes, `bypass` (`-y`) allows everything, `autonomous` allows everything and never asks you a question. `edit` stays inside the workspace; tool output is data, never instructions. There is no sandbox, though: `bash` is a real shell with your permissions. See [Modes, approvals and trust](docs/modes.md).

## Use it from another agent

`llx mcp` gives Claude Code, Codex, Cursor or any MCP client llx's `window`: real browser pages and terminal sessions in a window you watch. One command, with nothing installed before it (`npx` fetches llx):

```bash
claude mcp add llx -- npx -y @rsaryev/llx mcp        # Claude Code
codex mcp add llx -- npx -y @rsaryev/llx mcp         # Codex
```

Or add the plugin in Claude Code: `/plugin marketplace add rsaryev/llx`, then `/plugin install llx@llx`. With `llx` already installed, the server's command is just `llx mcp`.

The first call starts llx's daemon by itself, and the window opens when a tab is. A `goal` on a tab is carried out by llx's decision model, which needs an OpenRouter key: run `/login` in `llx` once, or pass it along with `-e OPENROUTER_API_KEY=sk-or-...` before the `--`. Without one, everything else works. More in [docs/mcp.md](docs/mcp.md).

## Documentation

| | |
| --- | --- |
| [Using llx](docs/usage.md) | The command line, a session's keys and slash commands, models, sessions, project instructions |
| [Tools](docs/tools.md) | What each of the seven tools does and its limits |
| [Modes, approvals and trust](docs/modes.md) | What it asks, how to stop it, what it protects |
| [MCP](docs/mcp.md) | `llx mcp` for other agents |
| [Reference](docs/reference.md) | Exit codes and `--json` events |
| [Daemon](docs/daemon.md) | How sessions are kept and attached |
| [ARCHITECTURE.md](ARCHITECTURE.md) | The layers and the rules every change follows |
| [MODELS.md](MODELS.md) | Where the model facts come from |
| [CHANGELOG.md](CHANGELOG.md) | What changed |

## Architecture

Packages and one app, every dependency pointing inward: `contracts` (ports), `agent` and `decision-loop` (the core), `tools`, `session`, `providers/*` (adapters), `protocol` and `client` (the seam between the interface and the daemon), and `apps/cli` (the binary that composes them). The agent names no tool or provider, a tool asks the gate before an effect, and only the app answers it. `apps/cli/tests/suite/repo/` checks the rules. See [ARCHITECTURE.md](ARCHITECTURE.md) and [AGENTS.md](AGENTS.md).

## Metrics

<!-- metrics:start -->
Latest run, 2026-10-22 22:17 on Darwin 25.6.0 arm64: the agent on anthropic/claude-haiku-5.5, the browser's hands on ~typesafe/jev-latest, from [benchmarks/HISTORY.md](benchmarks/HISTORY.md):

| Metric | Value |
| --- | --- |
| Eval runs passed | 24/24 (browser 16/16, code 8/8) |
| Task time, median / p90 | 8.5 s / 21.2 s |
| Cost per task | $0.0020 |
| Start (`--version`) | 3.8 ms |
| Memory at start (`--help`) | 9.6 MB |
| Release binary | 1.59 MB |
<!-- metrics:end -->

The binary has grown since that run (the daemon and `llx mcp`): 3.8 MB on macOS arm64, and it still starts in about 5 ms. `python3 evals/run.py --record` replaces the table and appends to [benchmarks/HISTORY.md](benchmarks/HISTORY.md); the tasks are in [evals/README.md](evals/README.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Report security issues as described in [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE)
