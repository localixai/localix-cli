# Evals

Live tasks against real sites, run through the release binary. Each task is judged by its exit code (which includes its `--expect` statements) and by strings its answer must contain.

```bash
cargo build --release
OPENROUTER_API_KEY=sk-or-... python3 evals/run.py                  # every task once
python3 evals/run.py --repeat 2 --jobs 4                           # stability
python3 evals/run.py --only todomvc,dynamic -- --model openai/gpt-5-mini
```

`--record` also appends the run to `benchmarks/runs/<date>.json` (tracked in git), regenerates `benchmarks/HISTORY.md` (every run, and per-task changes against the previous one) and the metrics table in the main README, so progress stays visible.

Every task makes paid OpenRouter calls (about $0.001 each with the defaults). Results and one trace per run land in `evals/results/<timestamp>/` (ignored by git); `summary.json` holds the table.

## Tasks

| Id | What it exercises |
| --- | --- |
| wiki-search | Typing into a search box with suggestions, submitting, a redirect |
| pizza-form | Text fields, a radio group, checkboxes, a time field, submit |
| login | A password field (typed, never read back) |
| dropdown | A native `<select>` whose page resets selection on every change |
| checkboxes | Bare checkboxes named only by the text beside them |
| dynamic | Waiting for content that appears after a delay |
| todomvc | Enter to add items, transparent styled checkboxes |
| hn-top | Following a link to another page |
| search-rust, gh-stars | Answering from the web without a start URL |
| wiki-chain | Following a page to another one and reading a fact |
| ddg-type | A search engine, then leaving its site |
| add-remove | Repeated clicks and counting |
| hn-three | Extracting a list |
| flights-oneway | Google Flights: cities with suggestions, a date picker, the cheapest result |
| flights-month | The cheapest day of a month (Larnaca → Tbilisi, asked in Russian), read from the date picker |
| flights-roundtrip | A round trip with two dates in a two-month picker |
| flights-ru | A direct flight on a date, asked in Russian, no start URL |
| flights-user | The cheapest day of a month, asked with typos, no start URL |
| schema-flights, schema-hn | `--schema`: the result as JSON matching a schema |
| game-2048 | Finding a 2048 site and scoring 500 with arrow keys, checked on the final page |

## Latest

See [benchmarks/HISTORY.md](../benchmarks/HISTORY.md).
