# Localix Surf

`lxsurf` is a goal-driven browser CLI. Give it a goal in plain words; it opens a native browser window, surfs until the goal is reached, prints the answer and exits.

```bash
$ lxsurf "Log in with username tomsmith and password SuperSecretPassword!" --url https://the-internet.herokuapp.com/login
   1.6s ▸ browser goal: Log in with username tomsmith and password SuperSecretPassword! · values: ["tomsmith","SuperSecretPassword!"]
   2.2s     TYPE_TEXT textbox "Username" [e1] ← "tomsmith" · 99%
   2.6s     TYPE_TEXT textbox "Password" [e2] ← "SuperSecretPassword!" · 99%
   3.0s     CLICK button "Login" [e3] · 98%
   3.4s     DONE · 97%
achieved in 5.1s · 4 actions · $0.0006
  time     2.5s planner · 1.6s decision model · 0.9s browser
  planner  anthropic/claude-haiku-5.5 · 2 calls · 4,479 in (2,026 cached) · 173 out · $0.0004
  decision typesafe/jev-1.14-20261014 · 4 calls · 4,880 in · 672 out · $0.0002
Logged in as tomsmith. The page shows "You logged into a secure area!"
```

- **Native and small.** The browser is the system WebView (WKWebView, WebView2, WebKitGTK) through `wry` and `tao`, the crates under Tauri. No bundled Chrome; the release binary is about 2 MB.
- **Two models, each for what it is good at.** A chat model plans: it sets short sub-goals, gives the exact strings to type, checks every page and writes the answer. A decision model ([Jev](https://openrouter.ai/docs/guides/community/jev)) does the clicking: one request per step, about 0.3 s, choosing an operation and an element among those observed. Model output never becomes a selector, coordinates or code.
- **Built for scripts and tests.** Stable exit codes, `--json`, `--expect` checks, `lxsurf check` for PASS/FAIL, invisible under CI, a fresh private session per run.

## Install

```bash
cargo install --git https://github.com/rsaryev/localix-surf
lxsurf login            # or: export OPENROUTER_API_KEY=sk-or-...
lxsurf doctor
```

Linux needs WebKitGTK (`libwebkit2gtk-4.1-0`) and a display (`xvfb-run` on servers). Windows needs the WebView2 runtime (preinstalled on Windows 11).

## Use

```bash
lxsurf "What is the current stable Rust release?"
lxsurf "Order a large pizza for Ada Lovelace, phone 555-0100, then submit" --url https://httpbin.org/forms/post
lxsurf check "Sign up with ada@example.com" --url http://localhost:3000 --expect "The welcome page is shown"
echo "a long goal" | lxsurf - --json
```

| Option | Meaning |
| --- | --- |
| `--url URL` | Start page (default: a site the goal names, else DuckDuckGo) |
| `--expect STATEMENT` | Must hold on the final page; repeatable; a false one fails the run |
| `--viewport V` | `desktop` (1280×860), `tablet` (820×1180), `mobile` (390×844) or `WxH`; tablet and mobile present as iOS Safari so sites serve their mobile layout. The planner can also switch it mid-run |
| `--schema S` | Return the result as JSON matching a JSON Schema (inline or `@file.json`): stdout carries the JSON alone, for pipelines |
| `--headless` / `--show` | Force the window invisible or visible (default: visible in a terminal, invisible under CI or when piped; `LXSURF_HEADLESS=1` makes invisible the default) |
| `--model ID` | Planner, any OpenRouter chat model with tools (env `LXSURF_MODEL`, default `anthropic/claude-haiku-5.5`) |
| `--decision-model ID` | System One model (env `LXSURF_DECISION_MODEL`, default `~typesafe/jev-latest`) |
| `--no-llm` | Decision model only: types verbatim spans of the goal, answers with the final page |
| `--json` | One JSON object on stdout (`schema: 1`), with per-model calls, tokens, cost and time |
| `--trace FILE` | Every planner call and decision step, with timestamps |
| `--max-turns N` / `--max-steps N` / `--timeout S` | Bounds: planner turns (50), actions per sub-goal (20), seconds (600) |
| `--dry-run` | Print the element table the decision model would see, no key needed |
| `--keep-open` | Leave the window open at the end |

Commands: `check`, `login`, `logout`, `models [--decision]`, `doctor`, `skills list|get`.

| Exit code | Meaning |
| --- | --- |
| 0 | Goal reached, every `--expect` held |
| 1 | Goal not reached, or an `--expect` was false |
| 2 | Turns or timeout ran out |
| 3 | Environment not ready (key, network, WebView, arguments) |
| 130 | Interrupted |

## For AI agents

Install the skill with `npx skills add rsaryev/localix-surf`, or add this repository as a Claude Code plugin marketplace. The installed skill is a stub; the guides ship inside the binary so they always match it:

```bash
lxsurf skills get core      # goals, JSON, models, trust boundaries
lxsurf skills get testing   # end-to-end checks and CI
```

## How it works

1. The window opens on the start page; `probe.js` is injected before any page script and answers over IPC with a per-run token the page cannot read.
2. The planner (one tool, `browser`) receives the goal and the page: visible text and a table of controls such as `[e4] textbox "City" [required]`.
3. A `browser` call can open a URL, run a sub-goal, run a script in the page, or read the whole text. A sub-goal is a loop: snapshot, one decision request with speculative heads (operation, click target, field, value, option), one action, settle. It stops on DONE, BLOCKED, a repeated or cycling action, another site, or its step limit, and returns the trace and the page.
4. The planner checks the page itself, sets the next sub-goal, and finally answers with `RESULT: achieved` or `not achieved`.

Design notes come from [Localix](https://github.com/rsaryev/localix)'s browser `goal` action and [jev-ultrafast](https://github.com/browser-use/jev-ultrafast).

## Metrics

<!-- metrics:start -->
Latest run, 2026-10-22 14:32 on Darwin 25.3.0 arm64 (anthropic/claude-haiku-5.5 + ~typesafe/jev-latest), from [benchmarks/HISTORY.md](benchmarks/HISTORY.md):

| Metric | Value |
| --- | --- |
| Eval tasks passed | 43/44 (22 tasks) |
| Task time, median / p90 | 9.9 s / 27.7 s |
| Cost per task (both models) | $0.0015 |
| Start (`--version`) | 33.7 ms |
| Cold start to first snapshot (example.com) | 1785.7 ms |
| Process memory | 81.3 MB |
| Release binary | 1.55 MB |
<!-- metrics:end -->

`python3 evals/run.py --record` runs the live tasks (forms, logins, dropdowns, checkboxes, TodoMVC, dynamic loading, search, multi-page chains, Google Flights searches, 2048, `--schema`), measures the binary, appends a dated file to `benchmarks/runs/`, regenerates [benchmarks/HISTORY.md](benchmarks/HISTORY.md) and this table. See [evals/README.md](evals/README.md).

## License

MIT
