#!/usr/bin/env python3
"""Runs evals/tasks.json through decision-loop and reports success, time and cost per task: the
decision loop with the decision model, each goal as a person would type it.

Usage: OPENROUTER_API_KEY=... python3 evals/run.py [--only id,id] [--repeat N] [--bin path] [-- extra decision-loop args]
Live: every task makes paid OpenRouter calls (about $0.0007 each) and opens real sites.
"""
import argparse, json, os, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

def run(job, binary, extra, trace_dir):
    index, task = job
    args = [binary, task["goal"], "--json", "--headless", "--timeout", "240"]
    if task.get("url"):
        args += ["--url", task["url"]]
    if task.get("schema"):
        args += ["--schema", json.dumps(task["schema"])]
    for statement in task.get("expect", []):
        args += ["--expect", statement]
    trace = os.path.join(trace_dir, f"{task['id']}-{index}.json")
    args += ["--trace", trace] + extra
    started = time.time()
    proc = subprocess.run(args, capture_output=True, text=True)
    wall = time.time() - started
    try:
        out = json.loads(proc.stdout.strip().splitlines()[-1])
    except Exception:
        out = {"status": "crash", "answer": proc.stderr[-300:], "usage": {}, "expects": []}
    answer = out.get("answer") or ""
    contains = all(s.lower() in answer.lower() for s in task.get("answer_contains", []))
    has_data = not task.get("schema") or isinstance(out.get("data"), dict)
    ok = proc.returncode == 0 and contains and has_data
    usage = out.get("usage", {})
    planner, decision = usage.get("planner") or {}, usage.get("decision") or {}
    stop = answer.split("\n", 1)[0].removeprefix("Stopped: ")
    return {"id": task["id"], "ok": ok, "stop": stop, "exit": proc.returncode, "status": out.get("status"),
            "wall": round(wall, 1), "turns": planner.get("calls"), "decisions": decision.get("calls"),
            "planner_s": round((planner.get("ms") or 0) / 1000, 1), "decision_s": round((decision.get("ms") or 0) / 1000, 1),
            "browser_s": round((usage.get("browser_ms") or 0) / 1000, 1),
            "cost": round(usage.get("cost_usd") or 0, 4),
            "tokens_per_decision": round((decision.get("input_tokens") or 0) / max(1, decision.get("calls") or 0)),
            "answer": answer[:160].replace("\n", " "), "expects": out.get("expects"), "trace": trace}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--only")
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument("--jobs", type=int, default=3)
    parser.add_argument("--bin", default=os.path.join(ROOT, "target/release/decision-loop"))
    parser.add_argument("--model", default=os.environ.get("DECISION_LOOP_MODEL"),
                        help="OpenRouter decision model (default: decision-loop's, or DECISION_LOOP_MODEL)")
    parser.add_argument("--record", action="store_true", help="add this run to benchmarks/ (history, README metrics)")
    parser.add_argument("extra", nargs="*")
    opts = parser.parse_args()
    if opts.model:
        opts.extra = ["--model", opts.model] + opts.extra
    tasks = json.load(open(os.path.join(ROOT, "evals/tasks.json")))
    if opts.only:
        keep = set(opts.only.split(","))
        tasks = [t for t in tasks if t["id"] in keep]
    trace_dir = os.path.join(ROOT, "evals/results", time.strftime("%Y%m%d-%H%M%S"))
    os.makedirs(trace_dir, exist_ok=True)
    jobs = [t for t in tasks for _ in range(opts.repeat)]
    with ThreadPoolExecutor(opts.jobs) as pool:
        results = list(pool.map(lambda job: run(job, opts.bin, opts.extra, trace_dir), enumerate(jobs)))
    for r in results:
        mark = "PASS" if r["ok"] else "FAIL"
        print(f"{mark} {r['id']:<17} {r['wall']:>5}s [jev {r['decision_s']}s · web {r['browser_s']}s] dec={r['decisions']} in/dec={r['tokens_per_decision']} ${r['cost']:<7}")
        if not r["ok"]:
            for e in r["expects"] or []:
                if not e.get("passed"):
                    print(f"     ✗ {e['text']} ({e.get('probability')})")
            if r.get("stop"):
                print(f"     stopped: {r['stop']}")
    passed = sum(r["ok"] for r in results)
    walls = sorted(r["wall"] for r in results)
    print(f"\n{passed}/{len(results)} passed · median {walls[len(walls)//2]}s · total ${sum(r['cost'] for r in results):.4f}")
    json.dump(results, open(os.path.join(trace_dir, "summary.json"), "w"), indent=2)
    if opts.record:
        sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
        import record
        models = {"decision": opts.model or "~typesafe/jev-latest"}
        print("recorded", record.record(results, opts.bin, models))
    sys.exit(0 if passed == len(results) else 1)

if __name__ == "__main__":
    main()
