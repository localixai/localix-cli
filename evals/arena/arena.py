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
  python3 evals/arena/arena.py --repeat 3 --record  # the benchmark: rows into benchmarks/arena/
  python3 evals/arena/arena.py --agents llx,codex   # some of them
  python3 evals/arena/arena.py --task phone-shop    # another task (TASKS)
"""
import argparse
import datetime
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = ROOT / "evals" / "arena"
KEEP = ROOT / "benchmarks" / "arena"
CACHE = Path.home() / ".cache" / "llx-arena"
TIMEOUT = 900

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
TASK_NAME = "ledger-web"
TASK, KEPT = TASKS[TASK_NAME]

# $ per million tokens, from models.dev (the catalog llx reads): input, cache read, cache write, output.
PRICES = {
    "gpt-6.1-terra": (1.0, 0.05, 1.25, 5.0),
    "gpt-6-luna": None,  # filled from the catalog at start (price_of)
    "claude-sonnet-5-5": (2.0, 0.1, 2.5, 10.0),
}
MODEL = {"llx": "gpt-6.1-terra", "codex": "gpt-6.1-terra", "claude": "claude-sonnet-5-5"}
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
    if agent == "llx":
        mine, theirs = Path.home() / ".config" / "llx", home / ".config" / "llx"
        files = ("openai.json", "models.json")
    else:
        mine, theirs = Path.home() / ".codex", home / ".codex"
        files = ("auth.json", "models_cache.json")
    theirs.mkdir(parents=True)
    for name in files:
        if (mine / name).exists():
            shutil.copy2(mine / name, theirs / name)
    return mine / files[0], theirs / files[0]


def hand_back(original, copy):
    """A refresh token is spent once it is used: a sign-in the run refreshed goes back to the
    person's own, or their sign-in would stop working."""
    if copy.exists() and copy.read_bytes() != original.read_bytes():
        shutil.copy2(copy, original)


def command(agent, work, home):
    model = MODEL[agent]
    if agent == "llx":
        llx = os.environ.get("LLX_BIN", str(ROOT / "target" / "release" / "llx"))
        env = {**plain_env(home), "LLX_NO_DAEMON": "1"}
        args = [llx, "exec", "--json", "-y", "-C", str(work), "--provider", "openai", "-m", model,
                "--reasoning", EFFORT, "--timeout", str(TIMEOUT), TASK]
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
    if agent == "llx":
        for e in events:
            if e.get("type") == "turn":
                row["turns"] += 1
                row["model"] = e.get("model")
            elif e.get("type") == "tool.call":
                row["tools"] += 1
            elif e.get("type") == "run.completed":
                spent = e["usage"]["agent"]
                row.update(input=spent["input_tokens"], cached=spent["cached_tokens"], output=spent["output_tokens"])
        row["cost"] = cost(MODEL[agent], row["input"] - row["cached"], row["cached"], 0, row["output"])
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
    shutil.copytree(ROOT / "evals" / "fixtures" / TASK_NAME, work, ignore=shutil.ignore_patterns("node_modules", "dist"))
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
            code = subprocess.run(args, cwd=work, env=env, stdin=subprocess.DEVNULL, stdout=out,
                                  stderr=err, timeout=TIMEOUT + 60).returncode
        except subprocess.TimeoutExpired:
            code = "timeout"
    seconds = round(time.monotonic() - started, 1)
    events = []
    for line in log.read_text().splitlines():
        try:
            events.append(json.loads(line))
        except ValueError:
            pass
    check = subprocess.run(["sh", str(HERE / "check.sh"), TASK_NAME, str(work), *KEPT], capture_output=True, text=True)
    row = {"agent": agent, "exit": code, "seconds": seconds, **measure(agent, events),
           "passed": check.returncode == 0, "check": check.stdout.strip().splitlines()}
    row["cost"] = round(row["cost"], 4)
    if row["model"] != MODEL[agent] and not str(row["model"] or "").startswith(MODEL[agent]):
        row["passed"] = False
        row["check"].append(f"FAIL ran on {row['model']}, not {MODEL[agent]}")
    shutil.rmtree(work / "node_modules", ignore_errors=True)
    return row


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
    original, copy = fresh_home(parent, agent) if agent in ("llx", "codex") else (None, None)
    try:
        row = run(agent, parent, logs)
    finally:  # a refreshed sign-in goes back; the copy never stays behind, even when a run breaks
        if copy:
            hand_back(original, copy)
        shutil.rmtree(parent / f"home-{agent}", ignore_errors=True)
    print(f"{n} {agent:7} {'pass' if row['passed'] else 'FAIL'} {row['seconds']:>6}s "
          f"{row['turns']:>3} turns {row['tools']:>3} tools {row['input']:>9,} in "
          f"{row['output']:>7,} out ${row['cost']:.4f}  {'; '.join(row['check'][:3])}", flush=True)
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--agents", default="llx,codex,claude")
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument("--record", action="store_true", help="append the rows to benchmarks/arena/")
    parser.add_argument("--model", help="the OpenAI model llx and Codex both use (default gpt-6.1-terra)")
    parser.add_argument("--effort", default=EFFORT)
    parser.add_argument("--task", default=TASK_NAME, choices=sorted(TASKS))
    parser.add_argument("--parallel", action="store_true", help="start every run at once")
    args = parser.parse_args()
    if args.model:
        MODEL["llx"] = MODEL["codex"] = args.model
    globals()["EFFORT"] = args.effort
    globals()["TASK_NAME"] = args.task
    globals()["TASK"], globals()["KEPT"] = TASKS[args.task]
    agents = args.agents.split(",")
    stamp = datetime.datetime.now().strftime("%Y-%m-%dT%H-%M-%S")
    logs = ROOT / "evals" / "results" / f"arena-{stamp}"
    logs.mkdir(parents=True)
    runs = [(n, agent) for n in range(args.repeat)
            for agent in agents[n % len(agents):] + agents[: n % len(agents)]]
    if args.parallel:
        with ThreadPoolExecutor(max_workers=len(runs)) as pool:
            rows = list(pool.map(lambda r: one(*r, logs), runs))
    else:
        rows = [one(n, agent, logs) for n, agent in runs]
    hidden = hashlib.sha256((HERE / "hidden" / f"{TASK_NAME}.test.tsx").read_bytes()).hexdigest()
    result = {"hidden_sha256": hidden, "at": stamp, "task": TASK_NAME, "parallel": args.parallel, "prompt": TASK, "effort": EFFORT, "models": MODEL, "versions": versions(), "rows": rows}
    (logs / "summary.json").write_text(json.dumps(result, indent=1))
    if args.record:
        KEEP.mkdir(parents=True, exist_ok=True)
        (KEEP / f"{stamp}.json").write_text(json.dumps(result, indent=1) + "\n")
    print(f"logs: {logs}")


if __name__ == "__main__":
    sys.exit(main())
