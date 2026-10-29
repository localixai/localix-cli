<p align="center">
  <img src="site/public/logo.svg" width="72" height="72" alt="llx">
</p>

<h1 align="center">llx</h1>

<p align="center">
  <strong>An autonomous agent runtime for CI and scripts</strong>
</p>

<p align="center">
  <code>llx exec</code> runs one task and ends with a <code>result</code> and an exit code your pipeline can branch on. One binary under 4 MB, on any model. A minimal agent at your terminal, too.
</p>

<p align="center">
  <a href="https://github.com/rsaryev/llx/actions/workflows/ci.yml"><img src="https://github.com/rsaryev/llx/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI status"></a>
  <a href="https://github.com/rsaryev/llx/releases/latest"><img src="https://img.shields.io/github/v/release/rsaryev/llx?color=black&label=release" alt="Latest release"></a>
  <a href="https://www.npmjs.com/package/@localixai/llx"><img src="https://img.shields.io/npm/v/@localixai/llx?color=black&label=npm" alt="npm version"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-black" alt="MIT License"></a>
  <img src="https://img.shields.io/badge/macOS%20%C2%B7%20Linux-black" alt="macOS and Linux">
  <img src="https://img.shields.io/badge/binary-under%204%20MB-black" alt="One binary under 4 MB">
  <img src="https://img.shields.io/badge/Rust-black?logo=rust" alt="Written in Rust">
</p>

<p align="center">
  <a href="#try-it">Try it</a> ·
  <a href="docs/usage.md">Documentation</a> ·
  <a href="#in-ci-and-scripts">In CI</a> ·
  <a href="#benchmark">Benchmark</a> ·
  <a href="https://llx.sh">llx.sh</a>
</p>

<p align="center">
  <a href="https://llx.sh">
    <img src="docs/assets/demo.gif" alt="llx at a terminal: /login picks OpenRouter and a model, Shift+Tab switches to bypass mode, then it fixes the failing median tests in stats.py and shows the diff, in 8 seconds for $0.002" width="880">
  </a>
</p>

llx is a model in a loop over five tools, on any provider: `bash`, `apply_patch`, `ask`, `agents` and `jobs`. Every task ends with a `result` call: whether the goal was achieved, a summary, what blocked it if not, and data matching your JSON Schema. Text alone never ends a task. One binary, three uses:

| Who calls | Command | What they get |
| --- | --- | --- |
| **Your CI and scripts** | `llx exec --json --schema …` | One task, then exit: the answer in the shape you define, JSON events, an exit code to branch on |
| **You** | `llx` | A minimal agent at your terminal |
| **Your other agents** | `llx exec` · `llx decide` | Commands they run from their shell: a helper on any model, a quick decision. They extend the agent you use, never replace it |

Every command and file change asks first, unless a rule allows it (`--allow 'bash(cargo test *)'`, or a mode of your own); `plan` runs read-only in the system's sandbox ([modes](docs/modes.md)). A task is bounded by `--max-turns`, `--timeout` and `--budget`: the first reached ends it. About 200 model providers: a ChatGPT sign-in, OpenRouter, or a key ([models](MODELS.md)).

## Try it

```bash
npm install -g @localixai/llx                # or the curl installer, below
cd your-project && llx                       # a session: /login first, then a task
echo '{"type":"object","properties":{"bump":{"enum":["patch","minor","major"]},"ship":{"type":"boolean"}},"required":["bump","ship"]}' > ship.json
git diff main... | llx exec --json --mode plan --schema @ship.json "Safe to ship? Which bump?" > events.jsonl
echo $?                                      # 0 achieved, 1 not achieved, 2 exhausted, 3 error
jq -c 'select(.type == "run.completed") | {status, data}' events.jsonl
# {"status":"achieved","data":{"bump":"patch","ship":true}}
```

## Benchmark

One task, llx and Codex on the same model (GPT-6 Luna, medium reasoning), every run started at once: build a task tracker's API on Bun from its tests, scored out of 199 (132 public tests, 66 hidden ones, the types). Medians.

| Agent | Runs | Score | Time | Cost | Finished |
| --- | --- | --- | --- | --- | --- |
| llx | 6 | 195 | 170 s | $0.026 | 6/6 |
| Codex | 3 | 197 | 143 s | $0.025 | 3/3 |

The runs: [benchmarks/task-tracker/runs](benchmarks/task-tracker/runs); the task and its checks: [evals/arena](evals/arena/arena.py).

## Install

```bash
npm install -g @localixai/llx
```

Or `curl -fsSL https://raw.githubusercontent.com/rsaryev/llx/main/scripts/install.sh | sh` (macOS and Linux x86_64, checked against `SHA256SUMS`), or `cargo install --locked --path apps/cli` from a clone. `llx doctor` checks the sign-ins, the models and the shell.

## In CI and scripts

In CI llx is the same agent as at your terminal, given one task. It works through the plain filesystem and primitive commands: `bash` with `gh`, `git`, `jq` and your project's own tools. There is no MCP server, no GitHub tool and no built-in prompt. Your workflow owns the prompt and the posting, so the same task runs on GitHub, GitLab or any runner with a shell. The job that runs the model holds a read-only token, and a second job posts what it answered: the write token is never near the model.

```yaml
# a step of a job, after actions/checkout
- uses: rsaryev/llx@v0
  id: llx
  env:
    OPENROUTER_API_KEY: ${{ secrets.OPENROUTER_API_KEY }}
    GH_TOKEN: ${{ github.token }} # read-only in this job
    PR: ${{ github.event.pull_request.number }}
  with:
    model: anthropic/claude-haiku-5.5
    allow: bash
    schema: '{"type":"object","properties":{"summary":{"type":"string"}},"required":["summary"]}'
    prompt: |
      Review pull request #$PR with gh and git. Run something to confirm a problem
      before you report it.
```

The answer is `steps.llx.outputs.result`, JSON matching your schema; the status, the spend and the exit code are outputs too.

How a task ends is a contract a pipeline can read. The exit code says it, and `--json` ends with a `run.completed` event carrying the same `status`:

| Exit code | `status` | Meaning |
| --- | --- | --- |
| 0 | `achieved` | `result` said the goal was reached |
| 1 | `not_achieved` | `result` said it was not (`blocked_by` says why), or its data did not match `--schema` |
| 2 | `exhausted` | Turns (`--max-turns`), time (`--timeout`) or money (`--budget`) ran out before a `result`: the first reached ends the task |
| 3 | `error` | No key, no network, a model error, a bad argument |
| 130 | `interrupted` | Ctrl-C or SIGTERM |

For a choice among a few options, `llx decide` asks a small decision model about stdin, in under a second for a fraction of a cent:

```console
$ git diff origin/main... | llx decide "Does this change break the public API?" --expect no
```

Ready-made GitHub workflows, each run for real: [pull request review](examples/github/review.yml) with inline comments, [`@llx` in any comment, issue or review thread](examples/github/llx.yml), [issue triage](examples/github/triage.yml), [a failed run explained](examples/github/ci-failure.yml), [a described pull request](examples/github/describe.yml), [a draft release](examples/github/release.yml) and [a weekly audit](examples/github/audit.yml): [examples/](examples/README.md). The action, the two-job pattern, rules, keys, limits, security and GitLab: [docs/ci.md](docs/ci.md). Shell recipes with `gh` and `glab`: [docs/recipes.md](docs/recipes.md).

## In other agents

Claude Code, another agent or a script of yours can call llx from its shell, as its task needs. `llx exec` hands a task to a helper on any provider and model, so the agent's own tokens go where they matter; `llx decide` answers a yes or no about some text in a second. See [docs/agents.md](docs/agents.md).

## Documentation

[Using llx](docs/usage.md) · [CI](docs/ci.md) · [Recipes](docs/recipes.md) · [Tools](docs/tools.md) · [Modes and trust](docs/modes.md) · [In other agents](docs/agents.md) · [Reference](docs/reference.md) · [Daemon](docs/daemon.md) · [Architecture](ARCHITECTURE.md) · [Models](MODELS.md) · [Changelog](CHANGELOG.md)

## Contributing

Start with [CONTRIBUTING.md](CONTRIBUTING.md) and [ARCHITECTURE.md](ARCHITECTURE.md). Report vulnerabilities as [SECURITY.md](SECURITY.md) describes. llx is open source under the [MIT License](LICENSE), made by [Localix](https://localix.ai).
