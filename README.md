<p align="center">
  <img src="site/public/logo.svg" width="72" height="72" alt="llx">
</p>

<h1 align="center">llx</h1>

<p align="center">
  <strong>More hands for the agent you already use</strong>
</p>

<p align="center">
  One binary under 4 MB that Claude Code, Codex, a script or CI calls from its shell: helpers on any model, one task for CI, and a choice made in a second. With a minimal agent of its own.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-black" alt="MIT License"></a>
  <img src="https://img.shields.io/badge/macOS%20%C2%B7%20Linux-black" alt="macOS and Linux">
  <img src="https://img.shields.io/badge/binary-under%204%20MB-black" alt="One binary under 4 MB">
  <img src="https://img.shields.io/badge/Rust-black?logo=rust" alt="Written in Rust">
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="docs/usage.md">Documentation</a> ·
  <a href="#in-other-agents">In other agents</a> ·
  <a href="#in-ci-and-scripts">In CI</a> ·
  <a href="https://llx.sh">llx.sh</a>
</p>

<p align="center">
  <a href="https://llx.sh">
    <img src="docs/assets/demo.gif" alt="llx at a terminal: /login picks OpenRouter and a model, Shift+Tab switches to bypass mode, then it fixes the failing median tests in stats.py and shows the diff, in 8 seconds for $0.002" width="880">
  </a>
</p>

llx is a minimal agent runtime: a model in a loop over a few plain tools, on any provider. llx runs three ways, from the same binary:

| Who calls | Command | What they get |
| --- | --- | --- |
| **You** | `llx` | A minimal agent at your terminal, with six tools: `read`, `edit`, `bash`, `ask`, `agents`, `jobs` |
| **Your scripts and CI** | `llx exec --json --schema …` | One task, then exit: JSON events, an answer in the shape you define, exit codes to branch on |
| **Your other agents** | `llx exec` · `llx decide` | Commands they run from their shell: helpers on any model, quick decisions. They extend Claude Code or Codex, never replace them |

<table>
  <tr>
    <td valign="top">
      <strong>A step in your pipeline</strong><br>
      A GitHub Action, and <code>llx exec</code> anywhere else: it answers to your JSON Schema and exits with a code CI can branch on.<br>
      <a href="docs/ci.md">CI →</a>
    </td>
    <td valign="top">
      <strong>Small on purpose</strong><br>
      Under 4 MB, starts in about 5 ms. About 200 model providers: a ChatGPT sign-in, OpenRouter, or a key.<br>
      <a href="MODELS.md">Models →</a>
    </td>
    <td valign="top">
      <strong>Reading never asks</strong><br>
      Every command and file change asks first, unless a rule allows it: <code>--allow 'bash(cargo test *)'</code>, or a mode of your own.<br>
      <a href="docs/modes.md">Modes →</a>
    </td>
  </tr>
</table>

## Install

```bash
npm install -g @localixai/llx
```

Or `curl -fsSL https://raw.githubusercontent.com/rsaryev/llx/main/scripts/install.sh | sh` (macOS and Linux x86_64, checked against `SHA256SUMS`), or `cargo install --locked --path apps/cli` from a clone.

```bash
cd your-project
llx                                    # a session: /login first, then a task
llx doctor                             # sign-ins, models, the shell
```

## In other agents

llx extends the agent you already use: it calls llx from its shell, as its task needs. `llx exec` hands a task to a helper on any provider and model, so the agent's own tokens go where they matter; `llx decide` answers a yes or no about some text in a second. See [docs/agents.md](docs/agents.md).

## In CI and scripts

In CI llx is the same agent as at your terminal. It works through the plain filesystem and primitive commands: `read`, `edit`, and `bash` with `gh`, `git`, `jq` and your project's own tools. There is no MCP server, no GitHub tool and no built-in prompt. Your workflow owns the prompt and the posting, so the same task runs on GitHub, GitLab or any runner with a shell. The job that runs the model holds a read-only token, and a second job posts what it answered: the write token is never near the model.

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
    deny: edit
    schema: '{"type":"object","properties":{"summary":{"type":"string"}},"required":["summary"]}'
    prompt: |
      Review pull request #$PR with gh and git. Run something to confirm a problem
      before you report it.
```

The answer is `steps.llx.outputs.result`, JSON matching your schema; the spend and the exit code are outputs too. For a choice among a few options, `llx decide` asks a small decision model about stdin, in under a second for a fraction of a cent:

```console
$ git diff origin/main... | llx decide "Does this change break the public API?" --expect no
```

Ready-made GitHub workflows, each run for real: [pull request review](examples/github/review.yml) with inline comments, [`@llx` in any comment, issue or review thread](examples/github/llx.yml), [issue triage](examples/github/triage.yml), [a failed run explained](examples/github/ci-failure.yml), [a described pull request](examples/github/describe.yml), [a draft release](examples/github/release.yml) and [a weekly audit](examples/github/audit.yml): [examples/](examples/README.md). The action, the two-job pattern, rules, keys, limits, security and GitLab: [docs/ci.md](docs/ci.md). Shell recipes with `gh` and `glab`: [docs/recipes.md](docs/recipes.md).

## Documentation

[Using llx](docs/usage.md) · [CI](docs/ci.md) · [Recipes](docs/recipes.md) · [Tools](docs/tools.md) · [Modes and trust](docs/modes.md) · [In other agents](docs/agents.md) · [Reference](docs/reference.md) · [Daemon](docs/daemon.md) · [Architecture](ARCHITECTURE.md) · [Models](MODELS.md) · [Changelog](CHANGELOG.md)

## Contributing

Start with [CONTRIBUTING.md](CONTRIBUTING.md) and [ARCHITECTURE.md](ARCHITECTURE.md). Report vulnerabilities as [SECURITY.md](SECURITY.md) describes. llx is open source under the [MIT License](LICENSE), made by [Localix](https://localix.ai).
