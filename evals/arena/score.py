#!/usr/bin/env python3
"""A scored arena task's result, in the folder an agent worked in: how many of the public tests
(test/) and of the hidden ones (hidden/<task>/, copied in now, after the agent has finished) pass,
and whether the types check. The files the task keeps must be as the original has them, or nothing
counts. Totals are the tests the original holds, so a suite that no longer loads scores zero, not
less of fewer. Prints one line per part, then `SCORE passed/total`; exit 0 when everything passes.

  python3 evals/arena/score.py task-tracker /path/to/work
"""
import filecmp
import os
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ElementTree
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HIDDEN = ROOT / "evals" / "arena" / "hidden"
# What the agent must leave as it found it: the tests and the project's setup.
KEPT = ["test", "package.json", "tsconfig.json", "bun.lock"]


def original(task):
    return ROOT / "benchmarks" / task / "original"


def same(a, b):
    if a.is_dir() != b.is_dir() or not a.exists() or not b.exists():
        return False
    if a.is_file():
        return filecmp.cmp(a, b, shallow=False)
    compared = filecmp.dircmp(a, b, ignore=[])
    if compared.left_only or compared.right_only or compared.funny_files:
        return False
    if any(not filecmp.cmp(a / f, b / f, shallow=False) for f in compared.common_files):
        return False
    return all(same(a / d, b / d) for d in compared.common_dirs)


def outcomes(work, suite):
    """Each test of `suite` (a path under `work`) by name: whether it passed."""
    with tempfile.TemporaryDirectory() as scratch:
        report = Path(scratch) / "report.xml"
        try:
            subprocess.run(["bun", "test", f"./{suite}", "--timeout", "5000", "--reporter=junit",
                            f"--reporter-outfile={report}"], cwd=work, capture_output=True, timeout=300)
        except subprocess.TimeoutExpired:
            return {}
        if not report.exists():
            return {}
        try:
            root = ElementTree.parse(report).getroot()
        except ElementTree.ParseError:
            return {}
        found = {}
        for case in root.iter("testcase"):
            name = f"{case.get('classname')} > {case.get('name')}"
            found[name] = not any(child.tag in ("failure", "error", "skipped") for child in case)
        return found


def totals(task):
    """How many tests each suite holds, counted on the original."""
    with tempfile.TemporaryDirectory() as scratch:
        work = Path(scratch) / "work"
        shutil.copytree(original(task), work, ignore=shutil.ignore_patterns("node_modules"))
        if (original(task) / "node_modules").exists():
            os.symlink(original(task) / "node_modules", work / "node_modules")
        shutil.copytree(HIDDEN / task, work / "hidden")
        return len(outcomes(work, "test")), len(outcomes(work, "hidden"))


def score(task, work):
    work = Path(work)
    public_total, hidden_total = totals(task)
    result = {"public": 0, "public_total": public_total, "hidden": 0, "hidden_total": hidden_total,
              "types": False, "kept": True, "failing": []}
    changed = [name for name in KEPT if not same(original(task) / name, work / name)]
    if changed:
        result["kept"] = False
        result["failing"] = [f"changed {name}" for name in changed]
    else:
        public = outcomes(work, "test")
        shutil.rmtree(work / "hidden", ignore_errors=True)
        shutil.copytree(HIDDEN / task, work / "hidden")
        try:
            hidden = outcomes(work, "hidden")
        finally:
            shutil.rmtree(work / "hidden", ignore_errors=True)
        result["public"] = min(sum(public.values()), public_total)
        result["hidden"] = min(sum(hidden.values()), hidden_total)
        result["failing"] = sorted(n for n, ok in {**public, **hidden}.items() if not ok)
        try:
            types = subprocess.run(["bunx", "tsc", "--noEmit"], cwd=work, capture_output=True, timeout=300)
            result["types"] = types.returncode == 0
        except subprocess.TimeoutExpired:
            pass
    result["score"] = result["public"] + result["hidden"] + int(result["types"])
    result["total"] = public_total + hidden_total + 1
    return result


def lines(result):
    out = [] if result["kept"] else [f"FAIL {f}: nothing counts" for f in result["failing"]]
    out.append(f"public {result['public']}/{result['public_total']}")
    out.append(f"hidden {result['hidden']}/{result['hidden_total']}")
    out.append(f"types {'ok' if result['types'] else 'FAIL'}")
    out.append(f"SCORE {result['score']}/{result['total']}")
    return out


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    result = score(sys.argv[1], sys.argv[2])
    print("\n".join(lines(result)))
    return 0 if result["score"] == result["total"] else 1


if __name__ == "__main__":
    sys.exit(main())
