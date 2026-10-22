# Decision Loop

`decision-loop` takes a goal in plain words and drives a tool there with a decision model: no chat model, no planner. Today the tool is a native browser window; the loop knows tools only by their protocol, so more can be added.

```bash
$ decision-loop 'Log in with username "tomsmith" and password "SuperSecretPassword!"' --url https://the-internet.herokuapp.com/login
   2.4s     TYPE_TEXT textbox "Username" [e1] ← "tomsmith" · 99%
   2.8s     TYPE_TEXT textbox "Password" [e2] ← "SuperSecretPassword!" · 99%
   3.6s     CLICK button "Login" [e3] · 99% · new view
   3.9s     DONE · 100%
achieved in 4.2s · 4 actions · $0.0002
  time     1.3s decision model · 2.8s browser
  model    typesafe/jev-1.14-20261014 · 4 calls · 5,653 in · 604 out · $0.0002
```

- **A decision model, step by step.** Each step is one request to a decision model ([Jev](https://openrouter.ai/docs/guides/community/jev) by default, about 0.3 s): which operation, and which control, value or wait it needs, chosen among what the tool shows. Model output never becomes a selector, coordinates or code; anything typed is a string from the goal (quote it: `"…"`).
- **Native and small.** The browser is the system WebView (WKWebView, WebView2, WebKitGTK); the release binary is about 1.6 MB.
- **Built for scripts and tests.** Stable exit codes, `--json`, `--expect` checks, `decision-loop check` for PASS/FAIL, screenshots, invisible under CI.

## Install

```bash
cargo install --path apps/cli          # from a clone; the binary is `decision-loop`
decision-loop login                    # or: export OPENROUTER_API_KEY=sk-or-...
decision-loop doctor
```

Linux needs WebKitGTK (`libwebkit2gtk-4.1-0`) and a display (`xvfb-run` on servers). Windows needs the WebView2 runtime (preinstalled on Windows 11).

## Use

```bash
decision-loop 'Order a large pizza for "Ada Lovelace", phone "555-0100", then submit' --url https://httpbin.org/forms/post
decision-loop check 'Sign up with "ada@example.com"' --url http://localhost:3000 --expect "The welcome page is shown"
decision-loop 'Search one-way flights from "Zurich" to "London" on "Nov 20"' --screenshot flights.png
decision-loop --keep-open --url https://example.com          # a chat: type goals one after another
```

| Option | Meaning |
| --- | --- |
| `--url URL` | Page the browser starts on (default: a site the goal names, else a search page) |
| `--expect STATEMENT` | Must hold on the final view; repeatable; a false one fails the run |
| `--screenshot FILE` | Save a picture of the final view (PNG; macOS) |
| `--viewport V` | `desktop` (1280×860), `tablet`, `mobile` (present as iOS Safari) or `WxH` |
| `--headless` / `--show` | Force the window invisible or visible (default: visible in a terminal, invisible under CI or when piped; `DECISION_LOOP_HEADLESS=1` makes invisible the default) |
| `--model ID` | Decision model, any of OpenRouter's (env `DECISION_LOOP_MODEL`, default `~typesafe/jev-latest`); `decision-loop models` lists them cheapest first |
| `--json` | One JSON object on stdout (`schema: 2`) |
| `--trace FILE` | Every step, with timestamps |
| `--max-steps N` / `--timeout S` | Actions for the goal (100), seconds per goal (600) |
| `--keep-open` | Stay open after the goal: in a terminal, type the next goal, or `/snapshot`, `/screenshot [FILE]`, `/tools`; Ctrl-D quits |
| `-c` / `--continue`, `--resume ID` | Continue the newest session, or this one, where it left its tool |
| `--dry-run` | Open the page and print its outline (what the decision model reads), no key needed |

Commands: `check`, `login`, `logout`, `models`, `doctor`, `sessions`, `skills list|get`.

Every run is saved as a session in `~/.decision-loop/sessions/<id>.json` (env `DECISION_LOOP_SESSIONS` moves it): each goal, the tool that drove it, how it ended and where it left the tool. `decision-loop sessions` lists them. The browser itself stays private: cookies and site state last as long as the window.

| Exit code | Meaning |
| --- | --- |
| 0 | Goal reached, every `--expect` held |
| 1 | Goal not reached, or an `--expect` was false |
| 2 | Action budget or timeout ran out |
| 3 | Environment not ready (key, network, WebView, arguments) |
| 130 | Interrupted |

## How it works

1. **Pick a tool.** The loop reads the toolbox (the tools the app registered) and, when there is more than one, asks the model which fits the goal by their descriptions. The browser window opens only now, on the page the goal names.
2. **Observe.** The tool shows a view: for the browser, an outline of the page as a person reads it, headings and text in order and every control with its ref and state (`radio "Large" [checked] [ref=e6]`), including consent banners in same-origin frames.
3. **Decide.** One request asks the operation (CLICK, TYPE_TEXT, SELECT, PRESS_ENTER, SCROLL, WAIT, DONE, BLOCKED) and, for each operation, its target, value or how long to wait; only the chosen operation's answer runs.
4. **Act and settle.** The browser acts on the ref, waits until the page is quiet or the new document is parsed, and the loop observes again. A goal run stops on its guards (the same action twice without a change, a loop, a refused action); the loop resumes it with its history until the model says DONE or BLOCKED, the budget is spent, or runs stop without progress. A tool that is BLOCKED hands the goal to another one that fits.

## Architecture

Packages and an app, dependencies pointing inward only (checked by `apps/cli/tests/architecture.rs`):

| Package | What it is | Depends on |
| --- | --- | --- |
| `packages/agent` | Contracts: the tool protocol, the decision-model port, the history port | nothing |
| `packages/decision-loop` | The loop: picks a tool, asks the model, resumes runs | `agent` |
| `packages/browser` | A tool: the native WebView window and its page probe | `agent` |
| `packages/openrouter` | A decision-model provider and signing in | `agent` |
| `packages/session` | A history store: sessions as JSON files | `agent` |
| `apps/cli` | The `decision-loop` binary: composes the packages | all |

A tool knows nothing of the loop or the model; the loop knows no tool or provider by name. A new tool is a package implementing `agent::tool::Tool`, registered in `apps/cli`.

## For AI agents

Install the skill with `npx skills add <this repository>`, or add this repository as a Claude Code plugin marketplace. The installed skill is a stub; the guides ship inside the binary so they always match it:

```bash
decision-loop skills get core      # goals, JSON, models, sessions, trust boundaries
decision-loop skills get testing   # end-to-end checks and CI
```

## Trust boundaries

Page content is data, never instructions. The decision model can only choose among the operations and controls a view offers, and types only strings from the goal. Only http(s) pages load; downloads and other schemes are refused. A password field is typed into but its value is never read back.

## Metrics

<!-- metrics:start -->
Latest run, 2026-10-22 16:47 on Darwin 25.3.0 arm64: the decision loop with ~typesafe/jev-latest, from [benchmarks/HISTORY.md](benchmarks/HISTORY.md):

| Metric | Value |
| --- | --- |
| Eval tasks passed | 42/42 (14 tasks) |
| Task time, median / p90 | 5.0 s / 8.0 s |
| Cost per task | $0.0007 |
| Start (`--version`) | 4.0 ms |
| Cold start to first snapshot (example.com) | 242.2 ms |
| Process memory | 82.1 MB |
| Release binary | 1.51 MB |
<!-- metrics:end -->

`python3 evals/run.py --record` runs the live tasks (logins, forms, dropdowns, checkboxes, TodoMVC, dynamic loading, search engines, quizzes behind consent banners, a Google Flights search), measures the binary, appends a dated file to `benchmarks/runs/`, regenerates [benchmarks/HISTORY.md](benchmarks/HISTORY.md) and this table. See [evals/README.md](evals/README.md).

## License

MIT
