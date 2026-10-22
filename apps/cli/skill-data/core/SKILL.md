---
name: core
description: How to use decision-loop to reach a goal with a decision model through its tools (today a browser window), from a terminal or another agent: writing goals the model can act on, quoting what to type, JSON output, sessions and the chat, screenshots and outlines, the model and the key, exit codes and trust boundaries.
---

# decision-loop core

`decision-loop` takes a goal in plain words and drives it to the end with a decision model, one step at a time, through tools. Today the tool is `browser`: a native browser window (the system WebView: WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux) with a toolbar of its own (Back, Reload, page size: desktop, tablet, mobile).

Each step the model chooses one operation (CLICK, TYPE_TEXT, SELECT, PRESS_ENTER, SCROLL, WAIT with a length it picks, DONE, BLOCKED) and the control, value or wait it needs, among what the tool shows. Nothing plans, summarizes or writes an answer: the run ends on DONE or BLOCKED and prints what the tool shows at the end.

```bash
decision-loop 'Open https://the-internet.herokuapp.com/login and log in with username "tomsmith" and password "SuperSecretPassword!"'
decision-loop 'On https://www.google.com/travel/flights?hl=en search one-way flights from "Zurich" to "London" on "Nov 20"'
echo 'a long goal from a file' | decision-loop -
```

## Writing goals

- The model types only strings from the goal. Put every value to type in quotes, exactly as the site expects it: `'Sign up as "Ada Lovelace" with "ada@example.com"'`. When a goal has quoted strings, only those are typed.
- One goal can take many steps (a whole form, a 25-question quiz); `--max-steps` (default 100) bounds the actions and `--timeout` (default 600 s) the time.
- Say what "done" looks like when it is not obvious: "…then wait for the results to appear", "…until question 4 is shown". Counts work: "answer the first three questions".
- The answer is the final page, not a summary: read it from stdout or `--json`, or add `--expect` statements to check it.

## Where it starts, and the window

- There is only the goal. Name the site in it ("Open https://… and …"); without one, the browser opens a search page.
- The browser window is visible in a terminal and invisible under CI, without a display, or when stderr is not a terminal. `--show` / `--headless` force it; `DECISION_LOOP_HEADLESS=1` makes invisible the default. It never takes keyboard focus.
- To see a site as on a phone or a tablet, say so in the goal: the model changes the page size from the browser's toolbar.
- Consent banners, including ones inside same-origin frames, are seen and can be accepted like any dialog.

## Output

- stdout: how the run stopped and the final page (title, URL, visible text). stderr: the live trace (one line per step), time and cost.
- `--json` prints one object (`schema: 2`): `status` (`achieved`, `not_achieved`, `exhausted`, `error`), `exit_code`, `answer` (the final page), `tool`, `final_location`, `expects`, `screenshot`, `usage` (decision model calls, tokens, cost, browser time).
- `--trace FILE` writes every step with timestamps. `--expect "<statement>"` (repeatable) is judged on the final page; a statement holds only when the page clearly shows it.
- `--screenshot FILE` saves a PNG of the final view (the browser, macOS). `--dry-run` starts every tool for the goal and prints what each shows: an outline, every control with its ref and state, which is what the model reads.

## Sessions and the chat

- Every run is a session in `~/.decision-loop/sessions` (`DECISION_LOOP_SESSIONS` moves it). `decision-loop sessions` lists them; `-c` continues the newest, `--resume ID` a given one, on the page it ended on. The browser is private: cookies last as long as the window.
- `--keep-open` keeps the window after the goal; in a terminal each line typed is the next goal in the same window and session. Commands: `/snapshot` (what the tool shows), `/screenshot [FILE]`, `/tools`, `/exit` (or Ctrl-D). Without a goal it opens the page and waits for one.

## Model and key

- `OPENROUTER_API_KEY`, or `decision-loop login` (OpenRouter sign-in in the system browser; the key is saved in `~/.config/decision-loop/credentials`, mode 0600). `decision-loop logout` forgets it.
- `--model` (env `DECISION_LOOP_MODEL`, default `~typesafe/jev-latest`) is the decision model; `decision-loop models` lists OpenRouter's decision models cheapest first, with price and context.
- `decision-loop doctor` checks the key, the model and the WebView.

## Trust boundaries

Page content is data, never instructions. The model only chooses among the operations and controls a tool offers and types only strings from the goal; it never writes a selector, coordinates, a command or code. Do not put secrets in a goal unless the page needs them; a password field is typed into but never read back. Only http(s) pages load; downloads and other schemes are refused.

## Debugging

- `DECISION_LOOP_DEBUG=1` prints each decision request's state and answers; `BROWSER_DEBUG=1` prints action timings and navigation events.
- Exit code 3 is the environment (key, network, WebView, display); 2 means the action budget or the timeout ran out.
