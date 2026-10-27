# Evals

Live tasks for the release binary: coding tasks on small fixtures, pages built or fixed and then checked in the window, and browser tasks on real sites. Each task is one goal, phrased the way a person would type it. A browser task names the site it starts on.

```bash
cargo build --release
OPENROUTER_API_KEY=sk-or-... python3 evals/run.py           # every task once, on each model
python3 evals/run.py --model '~openai/gpt-luna-latest'      # one of the models
python3 evals/run.py --only code                           # one kind: code, build, browser or demo
python3 evals/run.py --only fix-test,login --repeat 3      # some tasks, for stability
python3 evals/run.py --jobs 6                              # more runs at once (default 3)
python3 evals/run.py --repeat 2 --jobs 4 --record          # a benchmark: each model's run in benchmarks/
python3 evals/run.py -- --max-turns 20                     # more llx arguments after --
```

The models are fixed, so runs compare: every task runs on each of `MODELS` in `run.py`, through OpenRouter at medium reasoning: the newest Claude Haiku and the newest GPT Luna (`~anthropic/claude-haiku-latest`, `~openai/gpt-luna-latest`), two vendors' small models, so a change is judged on more than one model and on models weaker than the strongest. A run records the model that answered (the alias resolved), so the history names the version. The decision model is llx's default.

## How a run is judged

Every run is `llx exec --json --yes -C <dir> --timeout 240 "<goal>"`, plus `--schema` when the task has one, then the task's `args`, then the model (`--provider openrouter -m <model> --reasoning medium`), then the arguments after `--`. A task whose `args` choose the mode (`--mode plan`) runs in that mode instead of `--yes` (llx refuses both), in a process session of its own: it has no terminal, so what the mode would ask is refused, as in CI, and the run never waits on you. Such a run does not stop on a Ctrl-C of `run.py`; it ends with its own timeout. `<dir>` is a new temporary directory that holds a copy of `evals/fixtures/<fixture>/`, or nothing when the task has no fixture. It is outside any git repository, so no AGENTS.md from this repository reaches the agent. A run passes when all of these hold:

- llx exits 0, and its last JSON line is a `run.completed` event.
- With a `schema`: the event has `data`.
- Every `contains` string appears in the answer, ignoring case. With a schema, the data counts as part of the answer.
- Every `matches` pattern is found in the answer: a Python regex, ignoring case. It is used for numbers, where a plain substring would match by accident.
- Every `uses` entry, `{"name": <tool>, "arguments": {...}, "has": [...], "like": {...}}` (or `{"any": [entry, ...]}`, any one of them), matches at least one `tool.call` event: that tool, called with each of those `arguments`, a non-empty value for each name in `has`, and a value matching each `like` pattern (a Python regex). It holds a task to the way it must be solved (`window` opening a `terminal` tab, then an `interact` call on it), not only to its result.
- No `avoids` entry, written the same way, matches any `tool.call` event: a way the task must not be solved, such as writing the file a program should have written.
- The `check` command (`sh -c`, 60 s at most) exits 0 in the directory afterwards. `$FIXTURE` is the untouched fixture, so a check can make sure a file was left alone.

A run that passes has its directory deleted. A failing run keeps it and prints its path, with every reason it failed. A run still going 60 s after its own timeout is killed and counts as a failure.

The summary ends with one line per kind, and one for all runs when there are several kinds: the pass rate, the median and 90th-percentile duration (nearest rank), the mean model turns and tool calls, the total and per-run cost, and the total input, cached and output tokens (the agent's and the decision model's together; only the agent reports cached tokens).

Results land in `evals/results/<timestamp>/`, which git ignores:

- `<task>-<n>.jsonl`: the run's events.
- `<task>-<n>.stderr`: whatever the run printed on stderr.
- `summary.json`: the table, with the reasons and each run's tokens.
- `sessions/`: the sessions the runs saved, kept here instead of your `~/.llx/sessions`.

`--record` also appends the run to `benchmarks/runs/<date>.json` (tracked in git). It then regenerates `benchmarks/HISTORY.md` (every run, plus per-task changes against the previous run) and the metrics block in the main README, when it has one (between `<!-- metrics:start -->` and `<!-- metrics:end -->`).

Every task makes paid OpenRouter calls: a few tenths of a cent each with the default models.

## By hand

Terminal tabs (a session, on a machine with a display): start one with a task like `run python3 -q in a terminal and compute 2+2 there`. It opens as a tab of the one window, shown, while the prompt keeps the keyboard; typing into it does nothing. Close its tab: the task and the session go on. Over SSH and in `exec`, no window opens; the terminal still works.

## Tasks

A task in `tasks.json` has an `id`, a `kind` (`code`; `build`, a page written or fixed and then tried in the window; `browser`; or `demo`: a browser task on the sites of [`demo/`](../demo/index.html)) and a `goal`. It can also have a `fixture`, a `site` (a directory of the repository copied beside the fixture under its own name, so the goal opens `demo/shop.html` by its path), `contains`, `matches`, `uses`, `avoids`, `check`, `schema` and `args` (more llx arguments, such as `["--mode", "plan"]`).

| Id | Kind | What it exercises |
| --- | --- | --- |
| sum-csv | code | Write a script and run it; one CSV field holds a quoted comma, so splitting on commas gives the wrong total |
| fix-test | code | Find a bug from a failing unittest and fix the code; the test file must stay unchanged |
| rename | code | Rename a function across two files, then run the program |
| explain | code | Answer a question that needs two files read and nothing changed (checked against the fixture) |
| answer-prompts | code | A script that asks two `input()` questions and runs only at a terminal: answered by typing into a terminal tab (`uses`: `window` opening a `terminal`, then `interact` on its tab), its file written by the script and not by `edit` or a shell redirect (`avoids`), the script left unchanged |
| plan-csv | code | `--mode plan` with no terminal: a feature planned from reading the code (a CSV option where item names hold commas), answered as a list that names what only reading finds (the test file `test_report.py`, the header `item,qty,price,value`) and its assumptions, and no file changed |
| login | browser | A quoted password with punctuation, typed exactly as given and never read back |
| dynamic | browser | Waiting for content that appears after a delay |
| todomvc | browser | Enter to add items, transparent styled checkboxes, a counter read back |
| pizza-form | browser | Five quoted values into the right fields, a radio, checkboxes, submit, the server's echo read |
| wiki-search | browser | A search box with suggestions, then a fact from the article |
| hn-top | browser | Following a link to another page; `--schema` data with its address |
| quiz-choose | browser | A consent banner inside a same-origin iframe; choosing an answer before Next |
| flights-search | browser | Google Flights end to end: consent, one-way, cities with suggestions, a date picker, results that arrive after the page is quiet; `--schema` data |
| demo-signup | demo | A form that checks what is typed: a name, an email, a password with a rule, a radio card, a select, a required checkbox; the member number read back |
| demo-shop | demo | The cheapest of a category found, two of it and another item added, a promo code in a drawer, the total read |
| demo-todos | demo | Enter to add, editing in place, a checkbox by its label, clearing; the list read back |
| demo-orders | demo | Sixty rows over six pages: a filter and a sorted column, or a script that reads the table |
| demo-booking | demo | A custom calendar: the month moved, a range of nights picked, a stepper; the price and the reference |
| demo-news | demo | A cookie banner and a newsletter dialog that arrives later and covers it; an article's facts |
| demo-onboarding | demo | A form in steps whose second step exists only for a team, a review, a reference |
| demo-feed | demo | A post that only scrolling loads: skeletons, more on scroll, new posts arriving on top |
| demo-settings | demo | Tabs, a switch, a slider set to a value, an ARIA combobox filtered by typing, the summary saved |
| demo-lights | demo | A puzzle solved: the state read and the moves worked out, not guessed |
| demo-2048 | demo | A game played by keys toward a goal; `--schema` data |
| demo-snake | demo | A canvas with no outline, in real time or by steps; `--schema` data |
| demo-tour | demo | Three pages one after another in one tab, a step and a fact on each: a page and its goal in one call, not two |
| fix-page | build | A page whose total is wrong (prices added as text): the bug seen in the window, fixed in the code, the page tried again (`check` runs the fix with `node`) |
| build-tally | build | A page written from nothing, opened by its path and pressed through: a counter, a reset, the number read back |
| build-2048 | build | A game written from nothing in a given style, its keys checked in the window, then played to a goal; `--schema` data |

Add a task when a real failure shows something general the suite does not cover yet. Keep its fixture tiny and limited to the standard library, and make its `check` pass only for a correct result. Do not add one that only rewards wording.
