# Evals

Live tasks for the release binary: coding tasks on small fixtures, and browser tasks on real sites. Each task is one goal, phrased the way a person would type it. A browser task names the site it starts on.

```bash
cargo build --release
OPENROUTER_API_KEY=sk-or-... python3 evals/run.py           # every task once
python3 evals/run.py --only code                           # one kind: code or browser
python3 evals/run.py --only fix-test,login --repeat 3      # some tasks, for stability
python3 evals/run.py --jobs 6                              # more runs at once (default 3)
LLX_MODEL=<id> LLX_DECISION_MODEL=<id> python3 evals/run.py   # other models
python3 evals/run.py -- --max-turns 20                     # more llx arguments after --
```

## How a run is judged

Every run is `llx exec --json --yes -C <dir> --timeout 240 "<goal>"`, plus `--schema` when the task has one, then the task's `args`, then the arguments after `--`. A task whose `args` choose the mode (`--mode plan`) runs in that mode instead of `--yes` (llx refuses both), in a process session of its own: it has no terminal, so what the mode would ask is refused, as in CI, and the run never waits on you. Such a run does not stop on a Ctrl-C of `run.py`; it ends with its own timeout. `<dir>` is a new temporary directory that holds a copy of `evals/fixtures/<fixture>/`, or nothing when the task has no fixture. It is outside any git repository, so no AGENTS.md from this repository reaches the agent. A run passes when all of these hold:

- llx exits 0, and its last JSON line is a `run.completed` event.
- With a `schema`: the event has `data`.
- Every `contains` string appears in the answer, ignoring case. With a schema, the data counts as part of the answer.
- Every `matches` pattern is found in the answer: a Python regex, ignoring case. It is used for numbers, where a plain substring would match by accident.
- Every `uses` entry, `{"name": <tool>, "arguments": {...}, "has": [...], "like": {...}}`, matches at least one `tool.call` event: that tool, called with each of those `arguments`, a non-empty value for each name in `has`, and a value matching each `like` pattern (a Python regex). It holds a task to the way it must be solved (`bash` with `"tty": true`, then a `session` call with `input`), not only to its result.
- No `avoids` entry, written the same way, matches any `tool.call` event: a way the task must not be solved, such as writing the file a program should have written.
- The `check` command (`sh -c`, 60 s at most) exits 0 in the directory afterwards. `$FIXTURE` is the untouched fixture, so a check can make sure a file was left alone.

A run that passes has its directory deleted. A failing run keeps it and prints its path, with every reason it failed. A run still going 60 s after its own timeout is killed and counts as a failure.

The summary ends with one line per kind, and one for all runs when there are several kinds: the pass rate, the median and 90th-percentile duration (nearest rank), the mean model turns and tool calls, the total and per-run cost, and the total input, cached and output tokens (the agent's and the decision model's together; only the agent reports cached tokens).

Results land in `evals/results/<timestamp>/`, which git ignores:

- `<task>-<n>.jsonl`: the run's events.
- `<task>-<n>.stderr`: whatever the run printed on stderr.
- `summary.json`: the table, with the reasons and each run's tokens.
- `sessions/`: the sessions the runs saved, kept here instead of your `~/.lx/sessions`.

`--record` also appends the run to `benchmarks/runs/<date>.json` (tracked in git). It then regenerates `benchmarks/HISTORY.md` (every run, plus per-task changes against the previous run) and the metrics block in the main README.

Every task makes paid OpenRouter calls: a few tenths of a cent each with the default models.

## Tasks

A task in `tasks.json` has an `id`, a `kind` (`code` or `browser`) and a `goal`. It can also have a `fixture`, `contains`, `matches`, `uses`, `avoids`, `check`, `schema` and `args` (more llx arguments, such as `["--mode", "plan"]`).

| Id | Kind | What it exercises |
| --- | --- | --- |
| sum-csv | code | Write a script and run it; one CSV field holds a quoted comma, so splitting on commas gives the wrong total |
| fix-test | code | Find a bug from a failing unittest and fix the code; the test file must stay unchanged |
| rename | code | Rename a function across two files, then run the program |
| explain | code | Answer a question that needs two files read and nothing changed (checked against the fixture) |
| answer-prompts | code | A script that asks two `input()` questions and runs only at a terminal: answered by typing into a `tty` session (`uses`: the start, then a `session` call with `input`), its file written by the script and not by `edit` or a shell redirect (`avoids`), the script left unchanged |
| plan-csv | code | `--mode plan` with no terminal: a feature planned from reading the code (a CSV option where item names hold commas), answered as a list that names what only reading finds (the test file `test_report.py`, the header `item,qty,price,value`) and its assumptions, and no file changed |
| login | browser | A quoted password with punctuation, typed exactly as given and never read back |
| dynamic | browser | Waiting for content that appears after a delay |
| todomvc | browser | Enter to add items, transparent styled checkboxes, a counter read back |
| pizza-form | browser | Five quoted values into the right fields, a radio, checkboxes, submit, the server's echo read |
| wiki-search | browser | A search box with suggestions, then a fact from the article |
| hn-top | browser | Following a link to another page; `--schema` data with its address |
| quiz-choose | browser | A consent banner inside a same-origin iframe; choosing an answer before Next |
| flights-search | browser | Google Flights end to end: consent, one-way, cities with suggestions, a date picker, results that arrive after the page is quiet; `--schema` data |

Add a task when a real failure shows something general the suite does not cover yet. Keep its fixture tiny and limited to the standard library, and make its `check` pass only for a correct result. Do not add one that only rewards wording.
