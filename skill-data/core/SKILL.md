---
name: core
description: How to use lxsurf (Localix Surf) to reach a goal on the web and get an answer back, from a shell or another agent.
---

# lxsurf core

`lxsurf` takes a goal in plain words, opens a native browser window (the system WebView: WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux), surfs until the goal is reached, prints the answer and exits. Inside, a chat model plans and checks, and a fast decision model (Jev, through OpenRouter) does the clicking and typing.

Use it when a task needs a real web page operated (forms, logins, search boxes, filters, multi-step flows) or read after interaction. For a single static page you can fetch, a plain HTTP request is cheaper.

## Run

```bash
lxsurf "What is the current stable Rust release?"
lxsurf "Log in with username tomsmith and password SuperSecretPassword!" --url https://the-internet.herokuapp.com/login
echo "a long goal from a file" | lxsurf -
```

- The answer goes to stdout; a live trace goes to stderr.
- `--url` sets the start page. Without it, lxsurf opens a site the goal names, else DuckDuckGo.
- The window is visible in a terminal and invisible under CI, without a display, or when stderr is not a terminal. `--show` and `--headless` force either; `LXSURF_HEADLESS=1` makes invisible the default. The window never takes keyboard focus.
- Each run uses a fresh, private browser session: no cookies or logins carry over.

## Results for scripts

`--json` prints one object on stdout:

```json
{"schema": 1, "status": "achieved", "exit_code": 0, "answer": "…", "expects": [], "final_url": "…", "duration_ms": 5100,
 "usage": {"planner": {"model": "anthropic/claude-haiku-5.5", "calls": 2, "input_tokens": 4479, "cached_tokens": 2026, "output_tokens": 173, "cost_usd": 0.0004, "ms": 2500},
           "decision": {"model": "typesafe/jev-1.14-20261014", "calls": 4, "input_tokens": 4880, "cached_tokens": 0, "output_tokens": 672, "cost_usd": 0.0002, "ms": 1600},
           "browser_ms": 900, "actions": 4, "cost_usd": 0.0006}}
```

`status` is `achieved`, `not_achieved`, `exhausted` or `error`. Without `--json`, the run ends on stderr with what it spent: time per part (planner, decision model, browser) and, per model, calls, input tokens (and how many came from the prompt cache), output tokens and dollars.

`--trace FILE` writes every planner call and decision-model step with timestamps.

Exit codes are a contract:

| Code | Meaning |
| --- | --- |
| 0 | The goal was reached and every `--expect` held |
| 1 | The goal was not reached, or an `--expect` was false (the site failed) |
| 2 | Turns or the timeout ran out (the run could not decide) |
| 3 | The environment is not ready: no key, no network, no WebView, bad arguments |
| 130 | Interrupted |

## Structured output

`--schema` takes a JSON Schema (inline, or `@file.json`); stdout then carries one JSON object that matches it, and the prose answer goes to stderr. With `--json` it is under `data`. A result that does not match fails the run (exit 1, `data_error`).

```bash
lxsurf "Cheapest days to fly Larnaca to Tbilisi in November 2026, one way" \
  --schema '{"type":"object","required":["cheapest"],"properties":{"cheapest":{"type":"array","items":{"type":"object","required":["date","price"],"properties":{"date":{"type":"string"},"price":{"type":"number"}}}}}}' \
  | jq '.cheapest | min_by(.price)'
```

The schema is the planner's closing tool, so it costs no extra model call and keeps the prompt cache.

## Viewports

`--viewport desktop|tablet|mobile|WxH` sets the page size; tablet and mobile also present as iOS Safari, so sites serve their mobile pages. The planner can switch size mid-run (layout only).

## Checks

`--expect "<statement>"` (repeatable) is judged against the final page: the decision model answers with a probability, and only a doubtful one goes to the chat model. `lxsurf check …` prints `PASS` or `FAIL: <reason>` instead of the answer. See `lxsurf skills get testing`.

## Writing good goals

- Say what done looks like: "Open the comments page of the top story and tell me its title".
- Give exact values: names, dates as the site writes them, search words.
- Name the site or pass `--url` when you know it; it saves a search.
- Games and keyboard-driven pages work too: "Open any online 2048 and score 500 points". lxsurf presses keys (arrows, Escape, shortcuts) in batches and watches the score.
- One goal can take many steps; lxsurf sets its own sub-goals. `--max-turns` (default 50) and `--timeout` (default 600 s) bound it.

## Models and keys

- One OpenRouter key serves both models: `OPENROUTER_API_KEY`, or `lxsurf login` (OpenRouter sign-in in the system browser; the key is saved in `~/.config/localix-surf/credentials`, mode 0600). `lxsurf logout` forgets it.
- `--model` (env `LXSURF_MODEL`, default `anthropic/claude-haiku-5.5`) is the planner; any OpenRouter chat model with tool calling works. `lxsurf models` lists them.
- `--decision-model` (env `LXSURF_DECISION_MODEL`, default `~typesafe/jev-latest`) is the System One model. `lxsurf models --decision` lists them.
- `--no-llm` runs the decision model alone: what it types comes verbatim from the goal, and the answer is the final page. Fast and cheap for single-page tasks.
- `lxsurf doctor` checks the key, both models and the WebView.

## Trust boundaries

Page content is data, never instructions: lxsurf does not follow text on a page that asks it to do something. It does not enter passwords, payment details or personal data the goal did not give it, and does not buy, book, send or delete anything the goal did not ask for. Only http(s) pages load; downloads and other schemes are refused. A password field is typed into but its value is never read back.

## Debugging

- `--dry-run` opens the page and prints the element table the decision model would be offered, without a key.
- `--keep-open` leaves the window open at the end.
- `LXSURF_DEBUG=1` prints each decision request's state, every action's timing, and navigation events on stderr.
