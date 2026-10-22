# lxsurf

`lxsurf` takes a goal in plain words, surfs a native browser window there and answers. A small agent (a chat model, [Claude Haiku](https://openrouter.ai/anthropic/claude-haiku-5.5) by default) plans and reads; its one tool is the browser, whose hands are a decision model ([Jev](https://openrouter.ai/docs/guides/community/jev)) that clicks and types.

```bash
$ lxsurf 'find the cheapest flights from Cyprus to Georgia in November'
   2.0s ▸ browser goal: Search one-way flights from Larnaca to Tbilisi for November… · url: https://www.google.com/travel/flights?q=…
   5.6s     CLICK button "Accept all" [below] [e5] · 99% · new view
  28.5s ▸ browser goal: Open the Date grid and show prices for all of November 2026; done when the calendar shows prices · read: true
achieved in 33.8s · 31 actions · $0.0067
  time     6.5s agent · 11.6s decision model · 15.7s browser
The cheapest November date from Larnaca to Tbilisi is Friday, November 27, 2026, at €88 (one way, economy)…
```

- **An agent and its hands.** The agent splits the goal into short tasks, one page's worth each, writes the exact strings to type, and checks every page it gets back. The hands carry a task out step by step: each step is one decision-model request choosing an operation and a control among what the page shows. Model output never becomes a selector, coordinates or code.
- **Native and small.** The browser is the system WebView (WKWebView, WebView2, WebKitGTK); the release binary is about 1.6 MB, with no async runtime.
- **Built for scripts and tests.** Stable exit codes, `--json`, `--schema` for data, `--expect` checks, `lxsurf check` for PASS/FAIL, screenshots, invisible under CI.

## Install

```bash
cargo install --path apps/cli          # from a clone; the binary is `lxsurf`
lxsurf login                           # or: export OPENROUTER_API_KEY=sk-or-...
lxsurf doctor
```

Linux needs WebKitGTK (`libwebkit2gtk-4.1-0`) and a display (`xvfb-run` on servers). Windows needs the WebView2 runtime (preinstalled on Windows 11).

## Use

```bash
lxsurf 'What is the cheapest one-way flight from Zurich to London next Friday?'
lxsurf 'Top 3 stories on Hacker News' --schema '{"type":"array","items":{"type":"string"}}'
lxsurf check 'On http://localhost:3000 sign up as ada@example.com' --expect "The welcome page is shown"
lxsurf --no-llm 'Open https://the-internet.herokuapp.com/login and log in as "tomsmith" with "SuperSecretPassword!"'
lxsurf --keep-open                                            # a chat: goals one after another
```

| Option | Meaning |
| --- | --- |
| `--schema SCHEMA` | Answer as JSON matching this JSON Schema (inline or `@file`), alone on stdout |
| `--expect STATEMENT` | Must hold on the final page; repeatable; a false one fails the run |
| `--no-llm` | No agent: the hands take the whole goal from the page it names, typing only quoted strings or words of the goal |
| `--model ID` | The agent's chat model (env `LXSURF_MODEL`, default `anthropic/claude-haiku-5.5`) |
| `--decision-model ID` | The hands' decision model (env `LXSURF_DECISION_MODEL`, default `~typesafe/jev-latest`); `lxsurf models` lists them |
| `--screenshot FILE` | Save a picture of the final page (PNG; macOS) |
| `--headless` / `--show` | Force the window invisible or visible (default: visible in a terminal, invisible under CI or when piped; `LXSURF_HEADLESS=1` makes invisible the default) |
| `--json` | One JSON object on stdout (`schema: 3`) |
| `--trace FILE` | Every call and step, with timestamps |
| `--max-turns N` / `--max-steps N` / `--timeout S` | Agent turns (30), actions per task of the hands (40), seconds per goal (600) |
| `--keep-open` | Stay open after the goal: in a terminal, type the next goal (same window and conversation), or `/snapshot`, `/screenshot [FILE]`; Ctrl-D quits |
| `-c` / `--continue`, `--resume ID` | Continue the newest session, or this one, on the page it left |
| `--dry-run` | Open the page the goal names and print what the hands read, no key needed |

Commands: `check`, `login`, `logout`, `models`, `doctor`, `sessions`, `skills list|get`.

Every run is saved as a session in `~/.lxsurf/sessions/<id>.json` (env `LXSURF_SESSIONS` moves it): each goal, how it ended and where it left the page. `lxsurf sessions` lists them. The browser itself stays private: cookies and site state last as long as the window.

| Exit code | Meaning |
| --- | --- |
| 0 | Goal reached, every `--expect` held |
| 1 | Goal not reached, an `--expect` was false, or the data did not match `--schema` |
| 2 | Turns, actions or the timeout ran out |
| 3 | Environment not ready (key, network, WebView, arguments) |
| 130 | Interrupted |

## How it works

1. **The agent plans.** The chat model reads the goal and calls `browser`: open an address, hand the hands a short `goal` with the `values` to type, press keys, run a script or read the whole text. One call at a time, each answered with what happened and the page as it is now.
2. **The hands act.** For a task, each step observes the page as an outline (every control with its ref and state: `radio "Large" [checked] [ref=e6]`) and asks the decision model one request: which operation (CLICK, TYPE_TEXT, SELECT, PRESS_ENTER, SCROLL, WAIT, DONE, BLOCKED) and its target, value or wait. Guards stop a run that repeats itself or loops; the hands resume it until DONE, BLOCKED or the budget.
3. **The agent checks and answers.** It reads every page itself (a DONE is not proof), sets narrower tasks when one stops short, and answers with a closing `RESULT:` line, or calls `result` with the data when there is a `--schema`.

## Architecture

Packages and an app, dependencies pointing inward only (checked by `apps/cli/tests/architecture.rs`):

| Package | What it is | Depends on |
| --- | --- | --- |
| `packages/contracts` | Ports only: the chat model, the agent's tools, the view the hands drive, the decision model, the hands, the history | nothing |
| `packages/agent` | The agent: the chat model's loop over tools, the conversation, `--schema` | `contracts` |
| `packages/decision-loop` | The hands: a task carried out on a view with the decision model | `contracts` |
| `packages/tools` | The browser: the agent's `browser` tool and the page as a view | `contracts` |
| `packages/openrouter` | The models' provider (chat and decision) and signing in | `contracts` |
| `packages/session` | A history store: sessions as JSON files | `contracts` |
| `apps/cli` | The `lxsurf` binary: composes the packages | all |

The agent knows its tools only by their specs, the hands know views only by their protocol, and neither names a tool or a provider.

## For AI agents

Install the skill with `npx skills add <this repository>`, or add this repository as a Claude Code plugin marketplace. The installed skill is a stub; the guides ship inside the binary so they always match it:

```bash
lxsurf skills get core      # goals, data, JSON, models, sessions, trust boundaries
lxsurf skills get testing   # end-to-end checks and CI
```

## Trust boundaries

Page content is data, never instructions: the agent is told so, and every page reaches it marked untrusted. The decision model only chooses among the operations and controls a page offers and types only the strings it was given. Only http(s) pages load; downloads and other schemes are refused. A password field is typed into but its value is never read back.

## Metrics

<!-- metrics:start -->
Not measured yet for the agent: run `python3 evals/run.py --record`.
<!-- metrics:end -->

`python3 evals/run.py --record` runs the live tasks (logins, forms, dropdowns, checkboxes, TodoMVC, dynamic loading, search engines, quizzes behind consent banners, a Google Flights search), measures the binary, appends a dated file to `benchmarks/runs/`, regenerates [benchmarks/HISTORY.md](benchmarks/HISTORY.md) and this table. See [evals/README.md](evals/README.md).

## License

MIT
