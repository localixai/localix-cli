#!/usr/bin/env python3
"""The arena: a reference task, done by llx, Codex and Claude Code, each in a folder of its own.

Each run copies the task's fixture (evals/fixtures/<task>) into a new folder outside any repository
(so no AGENTS.md or CLAUDE.md reaches an agent), makes it a git repository, installs its packages,
then gives the agent the task's prompt word for word, fully automatic, with no user configuration: no rules, no
plugins, no MCP servers. The agent's JSON stream becomes one row: whether check.sh passed, the
wall time, the turns and tool calls, the tokens, and the cost priced from one table (PRICES, the
catalog's), API-equivalent: all three run on subscriptions here.

Runs go one after another (a benchmark is timed; see README.md), the agents' order rotating each
repeat; with --parallel every run starts at once, so all of them meet the backend in the same
minutes (its cache varies over the day), at the price of slower wall times on a shared machine. Usage:

  python3 evals/arena/arena.py                      # each agent once
  python3 evals/arena/arena.py --repeat 3 --record  # the benchmark: rows into benchmarks/<task>/runs/
  python3 evals/arena/arena.py --agents llx,codex   # some of them
  python3 evals/arena/arena.py --task phone-shop    # another task (TASKS)

A scored task (SCORED: task-tracker) starts from benchmarks/<task>/original instead, is judged by
score.py (public and hidden tests passed, out of all), and keeps each run's folder in the logs;
--record puts the folders and the rows under benchmarks/<task>/runs/<stamp>/.
"""
import argparse
import datetime
import hashlib
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import score as scoring

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / "evals" / "arena"
CACHE = Path.home() / ".cache" / "llx-arena"
TIMEOUT = 900
HANDBACK = threading.Lock()  # parallel runs hand sign-ins back one at a time

CHECKS = "Check your work with `bun test`, `bunx tsc --noEmit` and `bunx vite build`."
# Each task: its prompt, and the files the agent must leave as they are (check.sh compares them).
# ledger-web fixes an app against its spec; phone-shop builds a small landing page from a brief.
TASKS = {
    "ledger-web": (
        "The ledger app's tests fail and its report does not do what docs/SPEC.md says. Make the app "
        "match the spec, the month picker included. Do not change the existing tests. " + CHECKS,
        ["src/lib/csv.test.ts", "src/lib/report.test.ts"],
    ),
    "phone-shop": (
        "Build the landing page docs/BRIEF.md describes, for the phones in src/phones.ts. Do not "
        "change the existing test or src/phones.ts. " + CHECKS,
        ["src/App.test.tsx", "src/phones.ts"],
    ),
}
# A scored task starts from benchmarks/<task>/original (its tests, its setup) and is judged by
# score.py: how many public and hidden tests pass. Each run's folder is kept, to compare.
SCORED = {
    "task-tracker": "Build the task tracker's API in src/, in TypeScript on Bun, with no dependencies. The tests "
    "in test/ say what it does. Work until all of them pass: run `bun test` and `bunx tsc --noEmit`, fix "
    "what fails, and run them again, as many times as it takes. Split the code into modules: no file in "
    "src/ over 200 lines, no line over 120 characters (test/structure.test.ts checks it). Do not change or "
    "add files in test/, package.json, tsconfig.json or bun.lock.",
}
TASKS.update({name: (prompt, []) for name, prompt in SCORED.items()})
LONG = {"task-tracker": 1800}  # a timeout of its own, in seconds
TASK_NAME = "ledger-web"
TASK, KEPT = TASKS[TASK_NAME]

# $ per million tokens, from models.dev (the catalog llx reads): input, cache read, cache write, output.
PRICES = {
    "gpt-6.1-terra": (1.0, 0.05, 1.25, 5.0),
    "gpt-6-luna": None,  # filled from the catalog at start (price_of)
    "claude-sonnet-5-5": (2.0, 0.1, 2.5, 10.0),
}
# llx-openrouter: llx on the same model through OpenRouter (its key, not the ChatGPT sign-in).
MODEL = {"llx": "gpt-6.1-terra", "llx-openrouter": "openai/gpt-6.1-terra", "codex": "gpt-6.1-terra",
         "claude": "claude-sonnet-5-5"}
EFFORT = "low"


def plain_env(home):
    """The few variables an agent needs; nothing of this session's."""
    keep = {k: os.environ[k] for k in ("USER", "LOGNAME", "TERM", "LANG") if k in os.environ}
    path = ":".join(str(Path.home() / p) for p in (".bun/bin", ".local/bin")) + ":" + os.environ["PATH"]
    return {**keep, "HOME": str(home), "PATH": path}


def fresh_home(parent, agent):
    """A home of the run's own, beside its project and removed with it: only the sign-in and the
    model catalog are copied in, so no session, memory or log of an earlier run is there."""
    home = parent / f"home-{agent}"
    if agent.startswith("llx"):
        mine, theirs = Path.home() / ".config" / "llx", home / ".config" / "llx"
        files = ("credentials" if agent == "llx-openrouter" else "openai.json", "models.json")
    else:
        mine, theirs = Path.home() / ".codex", home / ".codex"
        files = ("auth.json", "models_cache.json")
    theirs.mkdir(parents=True)
    for name in files:
        if (mine / name).exists():
            shutil.copy2(mine / name, theirs / name)
    # The original's bytes as copied: hand_back must know whether anyone refreshed it since.
    snapshot = (mine / files[0]).read_bytes() if (mine / files[0]).exists() else None
    return mine / files[0], theirs / files[0], snapshot


def hand_back(original, copy, snapshot):
    """A refresh token is spent once it is used: a sign-in the run refreshed goes back to the
    person's own, or their sign-in would stop working. With parallel homes cloned from one
    sign-in, only a copy that changed is written, and only over the very bytes it was cloned from:
    if the original differs, another run already refreshed it, and that newer one stays."""
    if snapshot is None or not copy.exists():
        return
    with HANDBACK:
        if copy.read_bytes() != snapshot and original.exists() and original.read_bytes() == snapshot:
            shutil.copy2(copy, original)


def command(agent, work, home):
    model = MODEL[agent]
    if agent.startswith("llx"):
        llx = os.environ.get("LLX_BIN", str(ROOT / "target" / "release" / "llx"))
        provider = "openrouter" if agent == "llx-openrouter" else "openai"
        # openai=debug: each reply's routing in the log, kept beside the run (see `one`)
        env = {**plain_env(home), "LLX_NO_DAEMON": "1", "RUST_LOG": "warn,llx=info,openai=debug"}
        args = [llx, "exec", "--json", "-y", "-C", str(work), "--provider", provider, "-m", model,
                "--reasoning", EFFORT, "--timeout", str(LONG.get(TASK_NAME, TIMEOUT)),
                # Codex and Claude Code count no turns: the time limit alone bounds a run.
                "--max-turns", "10000", TASK]
        return args, env
    if agent == "codex":
        args = ["codex", "exec", "--json", "--ignore-user-config", "--ignore-rules", "--ephemeral",
                "--skip-git-repo-check", "--dangerously-bypass-approvals-and-sandbox", "-m", model,
                "-c", f'model_reasoning_effort="{EFFORT}"', "-C", str(work), TASK]
        return args, {**plain_env(home), "CODEX_HOME": str(home / ".codex")}
    # Claude Code with a configuration directory of its own: the person's CLAUDE.md, rules,
    # plugins and hooks stay out. It is signed in once (see README.md).
    env = {**plain_env(Path.home()), "CLAUDE_CONFIG_DIR": str(CACHE / "claude-config")}
    args = ["claude", "-p", "--model", model, "--effort", EFFORT, "--output-format", "stream-json",
            "--verbose", "--dangerously-skip-permissions", "--strict-mcp-config", "--mcp-config",
            '{"mcpServers":{}}', "--no-session-persistence", TASK]
    return args, env


def price_of(model):
    """A model's prices from the catalog llx keeps: models.dev's, as Codex's come from OpenAI."""
    catalog = json.loads((Path.home() / ".config" / "llx" / "models.json").read_text())
    for provider in ("openai", "anthropic"):
        found = (catalog.get(provider) or {}).get("models", {}).get(model)
        if found:
            text = found["cost"]["text"]
            cache = text["input"].get("cache", {})
            write = cache.get("write", 0)
            return (text["input"]["price"], cache.get("read", 0), write if isinstance(write, (int, float)) else write.get("5m", 0), text["output"]["price"])
    raise SystemExit(f"no price for {model} in the catalog")


def cost(model, fresh, read, write, out):
    if PRICES.get(model) is None:
        PRICES[model] = price_of(model)
    p_in, p_read, p_write, p_out = PRICES[model]
    return (fresh * p_in + read * p_read + write * p_write + out * p_out) / 1e6


def measure(agent, events):
    """One row's numbers from the agent's own events. Input counts every token sent."""
    row = {"model": None, "turns": 0, "tools": 0, "input": 0, "cached": 0, "output": 0, "cost": 0.0}
    if agent.startswith("llx"):
        # What the cache could give at most: each request what the one before it sent. A request
        # that reads less than its share was answered where its session's cache is not.
        # Past 100%: a first request found another session's start (the prompt, the tools).
        row["reachable"], before, row["largest"] = 0, 0, 0
        for e in events:
            if e.get("type") == "turn":
                row["turns"] += 1
                row["model"] = e.get("model")
                row["reachable"] += min(before, e.get("input_tokens", 0))
                before = e.get("input_tokens", 0)
                row["largest"] = max(row["largest"], before)
            elif e.get("type") == "tool.call":
                row["tools"] += 1
            elif e.get("type") == "run.completed":
                spent = e["usage"]["agent"]
                row.update(input=spent["input_tokens"], cached=spent["cached_tokens"], output=spent["output_tokens"])
                row["reported_cost"] = e["usage"].get("cost_usd")
        # At the API's prices, as Codex's: OpenRouter's own bill is reported_cost.
        row["cost"] = cost(MODEL[agent].removeprefix("openai/"), row["input"] - row["cached"], row["cached"], 0, row["output"])
        row["requests"] = row["turns"]
    elif agent == "codex":
        row["model"] = MODEL[agent]  # codex exec --json does not name it; -m chose it
        for e in events:
            if e.get("type") == "turn.completed":
                u = e["usage"]
                row["turns"] += 1
                row["input"] += u["input_tokens"]
                row["cached"] += u.get("cached_input_tokens", 0)
                row["output"] += u["output_tokens"]
            elif e.get("type") == "item.completed" and e["item"].get("type") not in ("agent_message", "reasoning"):
                row["tools"] += 1
        row["cost"] = cost(MODEL[agent], row["input"] - row["cached"], row["cached"], 0, row["output"])
        # Codex reports a task's totals only: a request for each tool call's result, and the first.
        row["requests"] = row["tools"] + 1
    else:
        write = 0
        for e in events:
            if e.get("type") == "system" and e.get("subtype") == "init":
                row["model"] = e.get("model")
            elif e.get("type") == "assistant":
                row["tools"] += sum(1 for c in e["message"].get("content", []) if c.get("type") == "tool_use")
            elif e.get("type") == "result":
                u = e["usage"]
                row["turns"] = e.get("num_turns", 0)
                write = u.get("cache_creation_input_tokens", 0)
                row["cached"] = u.get("cache_read_input_tokens", 0)
                row["input"] = u["input_tokens"] + write + row["cached"]
                row["output"] = u["output_tokens"]
                row["reported_cost"] = e.get("total_cost_usd")
        row["cost"] = cost(MODEL[agent], row["input"] - row["cached"] - write, row["cached"], write, row["output"])
    return row


def prepare(parent, agent):
    work = parent / agent
    fixture = scoring.original(TASK_NAME) if TASK_NAME in SCORED else ROOT / "evals" / "fixtures" / TASK_NAME
    shutil.copytree(fixture, work, ignore=shutil.ignore_patterns("node_modules", "dist"))
    quiet = {"stdout": subprocess.DEVNULL, "stderr": subprocess.DEVNULL, "check": True}
    subprocess.run(["git", "init", "-q"], cwd=work, **quiet)
    subprocess.run(["git", "add", "-A"], cwd=work, **quiet)
    subprocess.run(["git", "-c", "user.name=arena", "-c", "user.email=arena@localhost", "-c", "commit.gpgsign=false", "commit", "-qm", "start"], cwd=work, **quiet)
    subprocess.run(["bun", "install", "--frozen-lockfile"], cwd=work, **quiet)
    return work


def run(agent, parent, logs):
    work = prepare(parent, agent)
    args, env = command(agent, work, parent / f"home-{agent}")
    log = logs / f"{agent}-{parent.name}.jsonl"
    started = time.monotonic()
    with open(log, "w") as out, open(log.with_suffix(".stderr"), "w") as err:
        try:
            # Its own session: on a timeout the whole group dies, not only the agent's first process.
            code = scoring.run_group(args, LONG.get(TASK_NAME, TIMEOUT) + 60, cwd=work, env=env,
                                     stdin=subprocess.DEVNULL, stdout=out, stderr=err).returncode
        except subprocess.TimeoutExpired:
            code = "timeout"
    seconds = round(time.monotonic() - started, 1)
    events = []
    for line in log.read_text().splitlines():
        try:
            events.append(json.loads(line))
        except ValueError:
            pass
    if TASK_NAME in SCORED:
        scored = scoring.score(TASK_NAME, work)
        row = {"agent": agent, "exit": code, "seconds": seconds, **measure(agent, events),
               "passed": scored["score"] == scored["total"], "check": scoring.lines(scored),
               **{k: scored[k] for k in ("score", "total", "public", "public_total", "hidden", "hidden_total", "types", "kept")},
               "failing": scored["failing"], "work": f"{agent}-{parent.name}"}
        keep_work(work, logs / "work" / row["work"])
    else:
        check = subprocess.run(["sh", str(HERE / "check.sh"), TASK_NAME, str(work), *KEPT], capture_output=True, text=True)
        row = {"agent": agent, "exit": code, "seconds": seconds, **measure(agent, events),
               "passed": check.returncode == 0, "check": check.stdout.strip().splitlines()}
    row["cost"] = round(row["cost"], 4)
    row["uncached"] = row["input"] - row["cached"]  # what costs: the input read afresh
    if row["model"] != MODEL[agent] and not str(row["model"] or "").startswith(MODEL[agent]):
        row["passed"] = False
        row["check"].append(f"FAIL ran on {row['model']}, not {MODEL[agent]}")
    shutil.rmtree(work / "node_modules", ignore_errors=True)
    return row


def keep_work(work, to):
    """What the agent left, to read and compare: its files, not the installed packages, the
    repository or what the check wrote."""
    shutil.copytree(work, to, ignore=shutil.ignore_patterns("node_modules", ".git", "hidden", "check-*.log", "dist"))
    diff = subprocess.run(["git", "diff", "--stat", "HEAD"], cwd=work, capture_output=True, text=True).stdout
    added = subprocess.run(["git", "ls-files", "--others", "--exclude-standard"], cwd=work, capture_output=True, text=True).stdout
    (to.parent / f"{to.name}.diffstat").write_text(diff + ("\nnew files:\n" + added if added else ""))


def record_runs(stamp, logs, rows):
    """A scored task's benchmark: each run's folder beside its result, under the task's runs/."""
    home = ROOT / "benchmarks" / TASK_NAME / "runs" / stamp
    model = MODEL["llx"]
    counts = {}
    for row in rows:
        counts[row["agent"]] = counts.get(row["agent"], 0) + 1
        name = f"{row['agent']}-{counts[row['agent']]}"
        source = logs / "work" / row["work"]
        target = home / model / name
        row["folder"] = f"{model}/{name}"
        if not source.is_dir():  # a crash before the work was kept must not lose the other runs
            target.mkdir(parents=True)
            (target / "MISSING.md").write_text(
                f"# {row['agent']}: no work folder\n\nThe run ended (exit {row['exit']}) before its "
                f"work was kept, so there is nothing to show; its numbers are in result.json.\n")
            continue
        shutil.copytree(source, target)
        diffstat = source.parent / f"{row['work']}.diffstat"
        stat = diffstat.read_text() if diffstat.exists() else ""
        (target / "RESULT.md").write_text(result_page(row, stat))
    return home


def result_page(row, stat):
    failing = "\n".join(f"- {name}" for name in row["failing"]) or "- none"
    return (f"# {row['agent']} on {row['model']}\n\n"
            f"Score {row['score']}/{row['total']}: public tests {row['public']}/{row['public_total']}, "
            f"hidden {row['hidden']}/{row['hidden_total']}, types {'ok' if row['types'] else 'failing'}.\n\n"
            f"| Time | Requests | Input | Read afresh | Output | Cost |\n| --- | --- | --- | --- | --- | --- |\n"
            f"| {row['seconds']} s | {row['requests']} | {row['input']:,} | {row['uncached']:,} | {row['output']:,} | ${row['cost']:.4f} |\n\n"
            f"## Failing tests\n\n{failing}\n\n## Changed files\n\n```\n{stat.strip()}\n```\n")


def versions():
    def first(args):
        try:
            return subprocess.run(args, capture_output=True, text=True, timeout=20).stdout.strip().splitlines()[0]
        except (OSError, IndexError, subprocess.TimeoutExpired):
            return None
    llx = os.environ.get("LLX_BIN", str(ROOT / "target" / "release" / "llx"))
    return {"llx": first([llx, "--version"]), "codex": first(["codex", "--version"]),
            "claude": first(["claude", "--version"]), "bun": first(["bun", "--version"]),
            "machine": f"{platform.system()} {platform.machine()} {platform.mac_ver()[0]}".strip()}


def one(n, agent, logs):
    """One run in a folder and home of its own, its row printed as soon as it ends."""
    parent = Path(tempfile.mkdtemp(prefix=f"llx-arena-{n}-"))
    original = copy = snapshot = None
    try:
        if agent != "claude":
            original, copy, snapshot = fresh_home(parent, agent)
        row = run(agent, parent, logs)
    finally:  # a refreshed sign-in goes back; nothing stays behind, even when a run breaks
        if copy:
            hand_back(original, copy, snapshot)
        kept = parent / f"home-{agent}" / ".config" / "llx" / "logs" / "llx.log"
        if kept.exists():
            shutil.copy2(kept, logs / f"{agent}-{parent.name}.log")
        # The whole parent, work folder included: run() has kept what is kept (keep_work) by now.
        shutil.rmtree(parent, ignore_errors=True)
    scored = f"{row['score']}/{row['total']:<3} " if "score" in row else ""
    print(f"{n} {agent:14} {scored}{status(row):7} {row['seconds']:>6}s "
          f"{row['turns']:>3} turns {row['tools']:>3} tools {row['input']:>9,} in "
          f"{row['uncached']:>7,} afresh {row['output']:>7,} out ${row['cost']:.4f}{reach(row)}  "
          f"{'; '.join(row['check'][:3])}", flush=True)
    return row


def status(row):
    """crash: the agent did not end well (exit not 0, or the timeout); pass: everything passed;
    partial: it ended, but not everything passed."""
    if row["exit"] != 0:
        return "crash"
    return "pass" if row["passed"] and row.get("score", row.get("total")) == row.get("total") else "partial"


def median(values):
    return statistics.median(values)


def verdict(rows):
    """One line per agent over its rows (and, for a scored task, its most failing tests)."""
    lines = []
    for agent in dict.fromkeys(r["agent"] for r in rows):
        mine = [r for r in rows if r["agent"] == agent]
        crashes = sum(status(r) == "crash" for r in mine)
        scores = [r["score"] for r in mine if "score" in r]
        if scores:
            head = f"score mean {statistics.mean(scores):.1f} median {median(scores):g} min {min(scores)}"
        else:
            head = f"pass {sum(status(r) == 'pass' for r in mine)}/{len(mine)}"
        lines.append(f"{agent} n={len(mine)} {head} | time median {median([r['seconds'] for r in mine]):.0f}s"
                     f" | cost mean ${statistics.mean([r['cost'] for r in mine]):.3f} | crashes {crashes}/{len(mine)}")
        failing = Counter(name for r in mine for name in r.get("failing", []))
        if failing:
            lines.append("  fails most: " + "; ".join(f"{name} x{k}" for name, k in failing.most_common(3)))
    return lines


def delta(rows, previous):
    """Each agent's change against an earlier result (its `rows`), as medians and means."""
    lines = []
    for agent in dict.fromkeys(r["agent"] for r in rows):
        now = [r for r in rows if r["agent"] == agent]
        before = [r for r in previous["rows"] if r["agent"] == agent]
        if not before:
            continue
        parts = []
        scores, was = [r["score"] for r in now if "score" in r], [r["score"] for r in before if "score" in r]
        if scores and was:
            parts.append(f"median {median(scores) - median(was):+g}, mean {statistics.mean(scores) - statistics.mean(was):+.1f}")
        parts.append(f"time {median([r['seconds'] for r in now]) - median([r['seconds'] for r in before]):+.0f}s")
        parts.append(f"crashes {sum(status(r) == 'crash' for r in before)} -> {sum(status(r) == 'crash' for r in now)}")
        lines.append(f"{agent} vs {previous['at']}: " + ", ".join(parts))
    return lines


def previous_result(task, effort, model, hidden):
    """The newest recorded result of the same task, effort, model and hidden tests, else why none."""
    found = sorted((ROOT / "benchmarks" / task / "runs").glob("*/result.json"))
    if not found:
        return None, f"no recorded runs under benchmarks/{task}/runs/ to compare with"
    for path in reversed(found):
        try:
            old = json.loads(path.read_text())
        except ValueError:
            continue
        if (old.get("task"), old.get("effort"), old.get("models", {}).get("llx"), old.get("hidden_sha256")) == (task, effort, model, hidden):
            return old, ""
    return None, f"{len(found)} recorded run(s), none with the same effort, model and hidden tests"


def reach(row):
    """The cached input beside what the cache could give, where the agent's events say it."""
    if not row.get("reachable"):
        return ""
    return f"  cache {row['cached']:,}/{row['reachable']:,} ({100 * row['cached'] // row['reachable']}%)"


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--agents", default="llx,codex,claude")
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument("--record", action="store_true", help="keep the rows under benchmarks/<task>/runs/")
    parser.add_argument("--model", help="the OpenAI model llx and Codex both use (default gpt-6.1-terra)")
    parser.add_argument("--effort", default=EFFORT)
    parser.add_argument("--task", default=TASK_NAME, choices=sorted(TASKS))
    parser.add_argument("--parallel", action="store_true", help="start every run at once")
    parser.add_argument("--verdict", action="store_true", help="summarize each agent's rows (default with --repeat 2 or more)")
    parser.add_argument("--no-verdict", action="store_true", help="no summary after the table")
    args = parser.parse_args()
    if args.model:
        MODEL["llx"] = MODEL["codex"] = args.model
        MODEL["llx-openrouter"] = f"openai/{args.model}"
    globals()["EFFORT"] = args.effort
    globals()["TASK_NAME"] = args.task
    globals()["TASK"], globals()["KEPT"] = TASKS[args.task]
    agents = args.agents.split(",")
    stamp = datetime.datetime.now().strftime("%Y-%m-%dT%H-%M-%S")
    logs = ROOT / "evals" / "results" / f"arena-{stamp}"
    logs.mkdir(parents=True)
    runs = [(n, agent) for n in range(args.repeat)
            for agent in agents[n % len(agents):] + agents[: n % len(agents)]]
    if TASK_NAME in SCORED:
        files = sorted((HERE / "hidden" / TASK_NAME).iterdir())
        hidden = hashlib.sha256(b"".join(f.read_bytes() for f in files)).hexdigest()
    else:
        hidden = hashlib.sha256((HERE / "hidden" / f"{TASK_NAME}.test.tsx").read_bytes()).hexdigest()

    def summary(done):
        return {"hidden_sha256": hidden, "at": stamp, "task": TASK_NAME, "parallel": args.parallel, "prompt": TASK, "effort": EFFORT, "models": MODEL, "versions": versions(), "rows": done}
    finished, lock = [], threading.Lock()

    def tracked(n, agent):
        row = one(n, agent, logs)
        with lock:  # every finished row is on disk at once: Ctrl-C keeps what ended
            finished.append(row)
            (logs / "summary.json").write_text(json.dumps(summary(finished), indent=1))
        return row
    if args.parallel:
        with ThreadPoolExecutor(max_workers=len(runs)) as pool:
            rows = list(pool.map(lambda r: tracked(*r), runs))
    else:
        rows = [tracked(n, agent) for n, agent in runs]
    result = summary(rows)
    (logs / "summary.json").write_text(json.dumps(result, indent=1))
    if not args.no_verdict and (args.verdict or args.repeat >= 2):
        print("\n".join(verdict(rows)))
        old, why = previous_result(TASK_NAME, EFFORT, MODEL["llx"], hidden)
        print("\n".join(delta(rows, old)) if old else why)
    if args.record and TASK_NAME in SCORED:
        home = record_runs(stamp, logs, rows)
        (home / "result.json").write_text(json.dumps(result, indent=1) + "\n")
        print(f"recorded: {home}")
    elif args.record:
        keep = ROOT / "benchmarks" / TASK_NAME / "runs"
        keep.mkdir(parents=True, exist_ok=True)
        (keep / f"{stamp}.json").write_text(json.dumps(result, indent=1) + "\n")
    print(f"logs: {logs}")


if __name__ == "__main__":
    sys.exit(main())
