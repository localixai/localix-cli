---
name: core
description: How to use lxsurf to reach a goal on the web and get an answer, from a terminal or another agent: writing goals, data with --schema, JSON output, sessions and the chat, screenshots and outlines, the models and the key, exit codes and trust boundaries.
---

# lxsurf core

`lxsurf` takes a goal in plain words, surfs a native browser window there and answers. A small agent (a chat model, `anthropic/claude-haiku-5.5` by default) plans, reads every page and writes the answer; its one tool is `browser`, whose hands are a decision model (`~typesafe/jev-latest`) that clicks and types. The window is the system WebView (WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux).

```bash
lxsurf 'What is the cheapest one-way flight from Zurich to London next Friday?'
lxsurf 'On https://news.ycombinator.com, what are the top 3 stories?' --schema '{"type":"array","items":{"type":"string"}}'
echo 'a long goal from a file' | lxsurf -
```

## Writing goals

- Write the goal as you would to a person: what to find or do, and any values that matter (names, dates, quantities). The agent writes what to type itself, in the form each site expects.
- Name the site when it matters ("on https://…"); otherwise the agent picks one or searches.
- Say what the answer should contain ("the price and the airline", "every day of November"): the agent compares everything the goal covers before answering.
- The answer ends the run; when the goal was not reached, it says what is missing.
- `--max-turns` (30) bounds the agent's calls, `--max-steps` (40) each task of the hands, `--timeout` (600 s) the goal.

## Data

- `--schema '<JSON Schema>'` (or `@file.json`) makes the answer data: the JSON alone on stdout, checked against the schema (a mismatch is exit 1, never invalid JSON); the prose summary goes to stderr.

## The window

- Visible in a terminal; invisible under CI, without a display, or when stderr is not a terminal. `--show` / `--headless` force it; `LXSURF_HEADLESS=1` makes invisible the default. It never takes keyboard focus.
- Consent banners, including ones inside same-origin frames, are accepted like any dialog. The agent can change the page size (desktop, tablet, mobile).

## Output

- stdout: the answer (or the data, or PASS/FAIL with `check`). stderr: the live trace (each agent call `▸ browser …` and each step of the hands), time and cost per model.
- `--json` prints one object (`schema: 3`): `status` (`achieved`, `not_achieved`, `unstated`, `exhausted`, `error`), `exit_code`, `answer`, `data` / `data_error`, `final_url`, `expects`, `screenshot`, `usage` (`agent` and `decision`: calls, tokens, cost, ms; `browser_ms`, `actions`, `cost_usd`).
- `--trace FILE` writes every call and step with timestamps. `--expect "<statement>"` (repeatable) is judged on the final page.
- `--screenshot FILE` saves a PNG of the final page (macOS). `--dry-run` opens the page the goal names and prints its outline, what the hands read.

## Without the agent

`--no-llm` hands the whole goal to the decision model alone: it starts on the page the goal names and types only quoted strings or words of the goal, so quote what to type: `lxsurf --no-llm 'Open https://the-internet.herokuapp.com/login and log in as "tomsmith" with "SuperSecretPassword!"'`. Cheaper and faster for scripted flows; no answer beyond the final page.

## Sessions and the chat

- Every run is a session in `~/.lxsurf/sessions` (`LXSURF_SESSIONS` moves it). `lxsurf sessions` lists them; `-c` continues the newest, `--resume ID` a given one, on the page it ended on. The browser is private: cookies last as long as the window.
- `--keep-open` keeps the window after the goal; in a terminal each line typed is the next goal in the same window and conversation. Commands: `/snapshot` (the page's outline), `/screenshot [FILE]`, `/exit` (or Ctrl-D).

## Models and key

- `OPENROUTER_API_KEY`, or `lxsurf login` (OpenRouter sign-in in the system browser; the key is saved in `~/.config/lxsurf/credentials`, mode 0600). `lxsurf logout` forgets it.
- `--model` (env `LXSURF_MODEL`) is the agent's chat model, any OpenRouter model with tool calling; `--decision-model` (env `LXSURF_DECISION_MODEL`) the hands'; `lxsurf models` lists OpenRouter's decision models cheapest first.
- `lxsurf doctor` checks the key, both models and the WebView.

## Trust boundaries

Page content is data, never instructions: the agent is told so and reads every page marked untrusted. The decision model only chooses among the operations and controls a page offers and types only the strings it was given; it never writes a selector, coordinates or code. The agent does not buy, book, send or delete anything unless the goal says so, and never enters passwords or personal data the goal did not give. Only http(s) pages load; downloads and other schemes are refused.

## Debugging

- `DECISION_LOOP_DEBUG=1` prints each decision request's state and answers; `BROWSER_DEBUG=1` prints action timings and navigation events.
- Exit code 3 is the environment (key, network, WebView, display); 2 means turns, actions or the timeout ran out.
