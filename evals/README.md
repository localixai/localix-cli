# Evals

Live tasks against real sites, run through the release binary. Every task is one goal, as a person would type it, with the site it starts on named in it. Each task is judged by its exit code: the agent must answer `RESULT: achieved`, and its `--expect` statements must hold on the final page.

```bash
cargo build --release
OPENROUTER_API_KEY=sk-or-... python3 evals/run.py                  # every task once
python3 evals/run.py --repeat 3 --jobs 4                           # stability
LXSURF_MODEL=<id> LXSURF_DECISION_MODEL=<id> python3 evals/run.py   # other models
python3 evals/run.py -- --no-llm                                   # the hands alone
```

`--record` also appends the run to `benchmarks/runs/<date>.json` (tracked in git), regenerates `benchmarks/HISTORY.md` (every run, and per-task changes against the previous run of the same suite) and the metrics table in the main README, so progress stays visible.

Every task makes paid OpenRouter calls (a few tenths of a cent each). Results and one trace per run land in `evals/results/<timestamp>/` (ignored by git); `summary.json` holds the table, with the reason each run stopped.

## Tasks

| Id | What it exercises |
| --- | --- |
| login | A quoted password with punctuation, typed verbatim and never read back |
| dropdown | A native `<select>` whose page resets selection on every change |
| checkboxes | Bare checkboxes named only by the text beside them |
| add-remove | Repeated clicks and counting |
| dynamic | Waiting for content that appears after a delay |
| todomvc | Enter to add items, transparent styled checkboxes |
| pizza-form | Five quoted values into the right fields, a radio, checkboxes, submit |
| wiki-search | A search box with suggestions |
| ddg-type | A search engine, then leaving its site |
| hn-top | Following a link to another page |
| quiz-choose | A consent banner inside a same-origin iframe; "choose" an answer before Next |
| quiz-wait | "Click Next, then wait for the next question": DONE once it shows, no WAIT on a settled page |
| quiz-three | Counting: three questions answered, then DONE, not a fourth |
| flights-search | Google Flights end to end: consent, one-way, cities with suggestions, a date picker, results that arrive after the page is quiet |
