# llx in CI and scripts

`llx exec` runs one task without a session and exits. It is one binary under 4 MB that starts in about 5 ms, so a pipeline step does not wait on a runtime to install.

```bash
llx exec "is the changelog up to date with this branch?"   # the answer alone on stdout
```

## What a pipeline needs

- **A key.** The agent's model comes from `--provider` and `-m`, or `LLX_PROVIDER` and `LLX_MODEL`. Its key is the provider's variable from the model catalog (`OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`…). Keep it in the CI's secrets. A `goal` on a window tab needs an OpenRouter key whichever model the agent uses.
- **A mode.** A runner has no terminal, so nobody can approve anything:
  - `--mode default` (no flag) refuses every command, file change and page action.
  - `--mode plan` is read-only: files are read, and commands, file changes and page actions are refused. Use it for reviews and decisions.
  - `-y` (`--mode bypass`) allows everything. Use it for jobs that change the tree, on a branch, and only from triggers you trust.
- **Limits.** `--max-turns N` (default 50) and `--timeout SECONDS` (default 1800). A task that runs out of either exits 2.
- **A browser, if the task opens pages.** On Linux, the WebKitGTK runtime (`libwebkit2gtk-4.1-0`) and a display: run under `xvfb-run -a -s "-screen 0 1920x1080x24"`. Screenshots and comment pictures are macOS only.

## Answers a pipeline can read

| Flag | What it gives |
| --- | --- |
| (none) | The final answer alone on stdout |
| `--schema SCHEMA` | Data matching the JSON Schema (inline or `@file`), alone on stdout |
| `-o FILE` | The answer (or the data) also written to FILE |
| `--json` | One JSON event per line instead: calls, turns with their `cost_usd`, and `run.completed` with the usage ([reference](reference.md#json-events)) |

The exit code says how the task ended, and is a public contract:

| Code | Meaning |
| --- | --- |
| 0 | Achieved, or answered without saying it was not |
| 1 | The agent said it was not achieved, or the data did not match `--schema` |
| 2 | Turns or time ran out before an answer |
| 3 | Not ready: no key, no network, a model error, a bad argument |
| 130 | Interrupted |

## A decision step

When the answer is one of a few options and stdin holds all it needs, `llx decide` asks the
decision model alone: no agent, no tools, about a second and a fraction of a cent.

```bash
git diff origin/main... | llx decide "Does this change break the public API?" --expect no
git log -1 --format=%B | llx decide "What release is this?" \
  --choices "patch,minor=a new feature,major=a breaking change"
```

It prints the option it picked alone (`--json`: with each option's probability, the model and
the cost), exits 1 when `--expect` names another, 3 without an OpenRouter key.

When the step has to read the code or explain itself, run the agent read-only with a schema:

```bash
git diff origin/main... | llx exec --mode plan --max-turns 5 \
  --schema '{"type":"object","properties":{"breaking":{"type":"boolean"},"why":{"type":"string"}},"required":["breaking","why"]}' \
  "Does this change break the public API? The diff is on stdin." > verdict.json
jq -e '.breaking == false' verdict.json
```

Text a pipeline did not write (a pull request's body, an issue, a diff) goes on stdin or in a file, not in the prompt's words.

## GitHub Actions

```yaml
name: llx
on:
  pull_request:

permissions:
  contents: read

jobs:
  gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - run: npm install -g @localixai/llx
      - name: Is it breaking?
        id: gate
        env:
          OPENROUTER_API_KEY: ${{ secrets.OPENROUTER_API_KEY }}
          LLX_PROVIDER: openrouter
        run: |
          git diff "origin/${{ github.base_ref }}"... | llx exec --mode plan --max-turns 5 \
            --schema '{"type":"object","properties":{"breaking":{"type":"boolean"},"why":{"type":"string"}},"required":["breaking","why"]}' \
            "Does this change break the public API? The diff is on stdin." > verdict.json
          echo "breaking=$(jq -r .breaking verdict.json)" >> "$GITHUB_OUTPUT"
          jq -r .why verdict.json >> "$GITHUB_STEP_SUMMARY"
      - if: steps.gate.outputs.breaking == 'true'
        run: echo "::warning::llx thinks this change breaks the public API"
```

A job that fixes something runs with `-y` on a branch and opens a pull request with the diff; it never pushes to the base branch, and it runs only on triggers from people with write access, since `-y` allows every command.

```yaml
      - name: Fix the failing tests
        env:
          OPENROUTER_API_KEY: ${{ secrets.OPENROUTER_API_KEY }}
        run: llx exec -y --max-turns 30 "Run the tests and fix what fails. Change no test." -o answer.md
```

## Not yet

- No cost cap: `--max-turns` and `--timeout` bound a task, and `--json` reports its cost, but nothing stops it at an amount.
- No allowlist between `plan` and `-y` (for example "commands, but only `git diff`").
- No `llx-action`: the steps above install llx with npm.
