#!/usr/bin/env python3
"""The README's gif from the site's cast: one chapter of it, rendered with agg (brew install agg).

  scripts/demo/gif.py [CHAPTER] [OUT.gif]      # default: "fix the tests" → docs/assets/demo.gif
"""
import json
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CAST = os.path.join(ROOT, "site/public/demo-llx.cast")


def chapter(name):
    """The cast's events within the chapter `name`, as an asciinema v2 cast (what agg reads)."""
    lines = [l for l in open(CAST).read().split("\n") if l.strip()]
    head = json.loads(lines[0])
    events, at, on = [], 0.0, False
    for line in lines[1:]:
        gap, kind, data = json.loads(line)
        if kind == "m":
            if on:
                break
            on = data == name
            at = 0.0
            continue
        at += gap
        if on and kind == "o":
            events.append([round(at, 3), "o", data])
    if not events:
        raise SystemExit(f"no chapter {name!r} in {CAST}")
    # The chapter starts on the screen the one before it left: a clear first keeps it clean.
    events.insert(0, [0.0, "o", "\x1b[H\x1b[2J"])
    return {"version": 2, "width": head["term"]["cols"], "height": head["term"]["rows"]}, events


def main():
    name = sys.argv[1] if len(sys.argv) > 1 else "fix the tests"
    out = sys.argv[2] if len(sys.argv) > 2 else os.path.join(ROOT, "docs/assets/demo.gif")
    head, events = chapter(name)
    with tempfile.NamedTemporaryFile("w", suffix=".cast", delete=False) as cast:
        cast.write(json.dumps(head) + "\n")
        cast.writelines(json.dumps(event) + "\n" for event in events)
    subprocess.run(["agg", "--font-size", "15", "--line-height", "1.3", "--idle-time-limit", "2",
                    "--last-frame-duration", "3", cast.name, out], check=True)
    os.unlink(cast.name)
    print(f"{out}: {os.path.getsize(out) // 1024} KiB, {events[-1][0]:.0f} s")


if __name__ == "__main__":
    main()
