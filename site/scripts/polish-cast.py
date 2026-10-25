#!/usr/bin/env python3
"""Turns a raw asciinema recording of scripts/record-llx.exp into public/demo-llx.cast.

Drops the shell's setup before its `clear` and what follows the finished task. Before the task,
the person's pace is kept (up to 1.2 s between screens), so each picker and mode line can be
read; while the model works, a screen waits at most 0.06 s, so its thinking takes a second or
two. Pauses where there is something to read, and marks chapters for the player.
Usage: python3 scripts/polish-cast.py raw.cast public/demo-llx.cast
"""
import json
import re
import sys

src, dst = sys.argv[1], sys.argv[2]
lines = open(src).read().splitlines()
term = json.loads(lines[0])["term"]
events = [json.loads(l) for l in lines[1:]]
events = [e for e in events if e[1] == "o"]
start = next((i for i, e in enumerate(events) if "\x1b[3J" in e[2] or "\x1b[2J" in e[2]), 0)
events = events[start:]

CHAPTERS = [
    ("Model (", "Pick a provider and a model"),
    ("Mode: plan", "Modes: shift+tab"),
    ("median tests", "A task: fix the failing tests"),
]
# Where to stop and read: the picker, the mode lines, the finished task.
PAUSES = {"in all": 1.5, "Mode: bypass": 1.5}
LAST = 6.0
TYPING = 0.12
READ = 1.2
SCREEN = 0.06
# The task's own start and end: the person's pace before, the model's after, nothing past the end.
WORKING = "Working ("
END = "Worked for"

out, seen, tail, working, after = [], set(), "", False, 0
for i, (gap, _, text) in enumerate(events):
    # The footer's spend is the last thing the task draws: past it, the shell's exit.
    if END in tail and ("$0." in tail[-40:] or after > 3):
        break
    after += END in tail
    cap = TYPING if len(text) <= 2 else (SCREEN if working else READ)
    out.append([round(0.2 if i == 0 else min(gap, cap), 3), "o", text])
    # Matched on the text alone: the screen is drawn in pieces between escapes.
    tail = (tail + re.sub(r"\x1b\[[0-9;?]*[A-Za-z]", "", text))[-600:]
    working = working or WORKING in tail
    for key, label in CHAPTERS:
        if key not in seen and key in tail:
            seen.add(key)
            out.append([0, "m", label])
    for key, pause in PAUSES.items():
        if key not in seen and key in tail:
            seen.add(key)
            out.append([pause, "o", ""])

out.append([LAST, "o", ""])
with open(dst, "w") as f:
    f.write(json.dumps({"version": 3, "term": term, "title": "llx in a terminal"}) + "\n")
    f.writelines(json.dumps(e) + "\n" for e in out)
print(f"{sum(e[0] for e in out):.1f}s, {len(seen)} marks")
