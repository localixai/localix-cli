#!/usr/bin/env python3
"""Runs evals/tasks.json through lx and reports success, time and cost per task.

Usage: OPENROUTER_API_KEY=... python3 evals/run.py [--only id,kind] [--repeat N] [--jobs N]
       [--bin path] [--record] [-- extra lx args]

Each run is `lx --json --yes -C <dir> "<goal>"` in a fresh temporary directory (a copy of the
task's fixture, or empty), outside any git repository so no AGENTS.md reaches the agent. A run
passes when lx exits 0, the answer (and the data, with a schema) holds every `contains` string
and `matches` pattern, and the task's `check` command exits 0 in that directory afterwards.

Live: every task makes paid OpenRouter calls; browser tasks open real sites.
"""
import argparse, json, os, re, shutil, subprocess, sys, tempfile, time
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIXTURES = os.path.join(ROOT, "evals", "fixtures")
TIMEOUT_S = 240  # lx's own --timeout for one task
HANG_S = TIMEOUT_S + 60  # past this the process is killed: a run that never ends is a failure
CHECK_S = 60


def events(stdout):
    """The JSON lines lx printed; anything else on stdout is ignored."""
    parsed = []
    for line in stdout.splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if isinstance(event, dict):
            parsed.append(event)
    return parsed


def workdir(task):
    """A fresh directory for one run: the fixture copied in, or nothing."""
    path = tempfile.mkdtemp(prefix=f"lx-eval-{task['id']}-")
    if task.get("fixture"):
        shutil.copytree(os.path.join(FIXTURES, task["fixture"]), path, dirs_exist_ok=True)
    return path


def check(task, work):
    """Run the task's check in `work`; None when it passed, else why not."""
    command = task.get("check")
    if not command:
        return None
    env = dict(os.environ, FIXTURE=os.path.join(FIXTURES, task.get("fixture") or ""))
    try:
        proc = subprocess.run(["sh", "-c", command], cwd=work, env=env, capture_output=True,
                              text=True, timeout=CHECK_S, stdin=subprocess.DEVNULL)
    except subprocess.TimeoutExpired:
        return f"check timed out after {CHECK_S}s"
    if proc.returncode == 0:
        return None
    tail = (proc.stdout + proc.stderr).strip().splitlines()[-3:]
    return f"check exited {proc.returncode}" + (": " + " | ".join(tail) if tail else "")


def judge(task, code, done, work):
    """Every reason the run failed; empty when it passed."""
    reasons = []
    if code != 0:
        reasons.append(f"exit {code}" + (f" ({done.get('status')})" if done else " (no run.completed)"))
    if done and done.get("error"):
        reasons.append(done["error"])
    answer = (done or {}).get("answer") or ""
    if task.get("schema"):
        if done and "data" in done:
            answer += "\n" + json.dumps(done["data"], ensure_ascii=False)
        else:
            reasons.append("no data: " + str((done or {}).get("data_error", "none returned")))
    for text in task.get("contains", []):
        if text.lower() not in answer.lower():
            reasons.append(f"answer lacks {text!r}")
    for pattern in task.get("matches", []):
        if not re.search(pattern, answer, re.IGNORECASE):
            reasons.append(f"answer does not match /{pattern}/")
    failed_check = check(task, work)
    if failed_check:
        reasons.append(failed_check)
    return reasons


def run(job, binary, extra, out_dir):
    index, task = job
    work = workdir(task)
    args = [binary, "--json", "--yes", "-C", work, "--timeout", str(TIMEOUT_S)]
    if task.get("schema"):
        args += ["--schema", json.dumps(task["schema"])]
    args += extra + [task["goal"]]
    # Sessions go beside the results, not into the person's own ~/.lx/sessions.
    env = dict(os.environ, LX_SESSIONS=os.path.join(out_dir, "sessions"))
    started = time.time()
    try:
        proc = subprocess.run(args, capture_output=True, text=True, env=env, timeout=HANG_S,
                              stdin=subprocess.DEVNULL)
        code, stdout, stderr = proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired as hung:
        code = "hung"
        stdout = (hung.stdout or b"").decode(errors="replace") if isinstance(hung.stdout, bytes) else (hung.stdout or "")
        stderr = f"killed after {HANG_S}s"
    wall = time.time() - started
    trace = os.path.join(out_dir, f"{task['id']}-{index}.jsonl")
    with open(trace, "w") as f:
        f.write(stdout)
    if stderr.strip():
        with open(trace[:-len(".jsonl")] + ".stderr", "w") as f:
            f.write(stderr)
    seen = events(stdout)
    done = next((e for e in reversed(seen) if e.get("type") == "run.completed"), None)
    reasons = judge(task, code, done, work)
    if done is None and stderr.strip():
        reasons.append(stderr.strip().splitlines()[-1][:200])
    tools = {}
    for event in seen:
        if event.get("type") == "tool.call":
            tools[event.get("name")] = tools.get(event.get("name"), 0) + 1
    started_event = next((e for e in seen if e.get("type") == "run.started"), {})
    usage = (done or {}).get("usage") or {}
    agent, decision = usage.get("agent") or {}, usage.get("decision") or {}
    ok = not reasons
    if ok:
        shutil.rmtree(work, ignore_errors=True)
    return {"id": task["id"], "kind": task.get("kind", "browser"), "ok": ok, "exit": code,
            "status": (done or {}).get("status"), "wall": round(wall, 1),
            "turns": agent.get("calls"), "tool_calls": sum(tools.values()), "tools": tools,
            "decisions": decision.get("calls"),
            "agent_s": round((agent.get("ms") or 0) / 1000, 1),
            "decision_s": round((decision.get("ms") or 0) / 1000, 1),
            "cost": round(usage.get("cost_usd") or 0, 4),
            "answer": ((done or {}).get("answer") or "")[:160].replace("\n", " "),
            # What the run itself says it used: the flags, the environment and the defaults.
            "model": started_event.get("model"), "decision_model": started_event.get("decision_model"),
            "reasons": reasons, "trace": trace, "work": None if ok else work}


def used(results, key):
    """The model the runs reported in `run.started`, or each of them when they differ."""
    seen = sorted({r[key] for r in results if r.get(key)})
    return ", ".join(seen) if seen else "unknown"


def select(tasks, only):
    """Tasks whose id or kind is in `only` (comma-separated), or all of them."""
    if not only:
        return tasks
    keep = set(only.split(","))
    chosen = [t for t in tasks if t["id"] in keep or t.get("kind", "browser") in keep]
    unknown = keep - {t["id"] for t in tasks} - {t.get("kind", "browser") for t in tasks}
    if unknown:
        sys.exit(f"unknown task or kind: {', '.join(sorted(unknown))}")
    return chosen


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--only", help="comma-separated task ids or kinds (code, browser)")
    parser.add_argument("--repeat", type=int, default=1, help="runs per task")
    parser.add_argument("--jobs", type=int, default=3, help="runs at once")
    parser.add_argument("--bin", default=os.path.join(ROOT, "target/release/lx"))
    parser.add_argument("--record", action="store_true", help="add this run to benchmarks/ (history, README metrics)")
    parser.add_argument("extra", nargs="*", help="after --: more lx arguments, such as -m MODEL")
    opts = parser.parse_args()
    if not os.access(opts.bin, os.X_OK):
        sys.exit(f"{opts.bin} is missing: cargo build --release, or pass --bin")
    with open(os.path.join(ROOT, "evals/tasks.json")) as f:
        tasks = select(json.load(f), opts.only)
    out_dir = os.path.join(ROOT, "evals/results", time.strftime("%Y%m%d-%H%M%S"))
    os.makedirs(out_dir, exist_ok=True)
    jobs = list(enumerate(t for t in tasks for _ in range(opts.repeat)))
    with ThreadPoolExecutor(max(1, opts.jobs)) as pool:
        results = list(pool.map(lambda job: run(job, opts.bin, opts.extra, out_dir), jobs))
    for r in results:
        mark = "PASS" if r["ok"] else "FAIL"
        print(f"{mark} {r['id']:<15} {r['wall']:>6}s [agent {r['agent_s']}s, hands {r['decision_s']}s] "
              f"turns={r['turns']} tools={r['tool_calls']} dec={r['decisions']} ${r['cost']:<7} {r['answer'][:60]}")
        for reason in r["reasons"]:
            print(f"     - {reason}")
        if r["work"]:
            print(f"     - left in {r['work']}")
    passed = sum(r["ok"] for r in results)
    walls = sorted(r["wall"] for r in results)
    median = walls[len(walls) // 2] if walls else 0
    print(f"\n{passed}/{len(results)} passed, median {median}s, total ${sum(r['cost'] for r in results):.4f}"
          f"\nresults in {out_dir}")
    with open(os.path.join(out_dir, "summary.json"), "w") as f:
        json.dump(results, f, indent=2)
    if opts.record:
        sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
        import record
        models = {"agent": used(results, "model"), "decision": used(results, "decision_model")}
        print("recorded", record.record(results, opts.bin, models))
    sys.exit(0 if passed == len(results) else 1)


if __name__ == "__main__":
    main()
