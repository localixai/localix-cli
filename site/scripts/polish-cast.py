#!/usr/bin/env python3
"""Turns a raw asciinema recording of scripts/record-llx.exp into public/demo-llx.cast.

Drops the shell's setup before its `clear` and its exit. Typing keeps a person's pace (a key
waits at most 0.1 s); a screen waits at most 1.2 s for the next, so a picker can be read and the
agent's calls come one by one. While the agents work the clock runs faster. Pauses after each
answer so it can be read, and marks a chapter where each part starts. The recording account's
name (a file listing shows it) becomes `user`, and the demo's home, the parent of the folder
given third, becomes `~`.
Usage: python3 scripts/polish-cast.py raw.cast public/demo-llx.cast [folder]
"""
import getpass
import json
import os
import re
import sys

src, dst = sys.argv[1], sys.argv[2]
lines = open(src).read().splitlines()
term = json.loads(lines[0])["term"]
events = [json.loads(l) for l in lines[1:]]
ME = getpass.getuser()
HOME = os.path.dirname(sys.argv[3].rstrip("/")) if len(sys.argv) > 3 else None


def clean(text):
    """The demo's home is `~`, as its prompt says, and the account's name hidden."""
    if HOME:
        text = text.replace(HOME, "~")
    return text.replace(ME, "user")


events = [[g, k, clean(t)] for g, k, t in events if k == "o"]
start = next((i for i, e in enumerate(events) if "\x1b[3J" in e[2] or "\x1b[2J" in e[2]), 0)
events = events[start:]

CHAPTERS = [("/model", "/model"), ("Start two agents", "agents"), ("llx exec -y", "ship check"), ("llx decide", "decide"), ("commit message", "commit message")]
# The agents' work, from the task to its end line, plays this much faster.
FAST = ("Start two agents", "Worked for", 0.4)
PROMPT = re.compile(r"stats\S* \$ $")
READ = 2.5
LAST = 6.0
TYPING = 0.1
OUTPUT = 1.2


def typed(text):
    """zsh echoes a command sent at once as one chunk: its first key, a backspace, the line (with
    its own redraw where the line wraps), then the end of the edit. The line is played back a key
    at a time, at a steady typing pace; the terminal wraps it as it goes."""
    if len(text) < 3 or text[1] != "\x08":
        return None
    line, end, rest = text[2:].partition("\x1b[?2004l")
    line = re.sub(r" \r\x1b\[K.\r", "", line)
    if "\x1b" in line:
        return None
    keys = [[0.04 + 0.03 * (i % 3), c] for i, c in enumerate(line)]
    if end:
        keys.append([0.3, end + rest])
    return keys


out, seen, tail, prompts, pace = [], set(), "", 0, 1.0
for i, (gap, _, text) in enumerate(events):
    if "exit" in text and i > len(events) - 4:
        break
    keys = typed(text)
    if keys:
        out.append([0.6, "o", ""])
        out.extend([g, "o", c] for g, c in keys)
    else:
        cap = TYPING if len(text) <= 2 else OUTPUT
        out.append([round(0.2 if i == 0 else min(gap, cap) * pace, 3), "o", text])
    plain = re.sub(r"\x1b\[[0-9;?]*[A-Za-z]", "", text)
    tail = (tail + plain)[-600:]
    if pace == 1.0 and FAST[0] in seen and FAST[1] not in seen:
        pace = FAST[2]
    if pace != 1.0 and FAST[1] in plain:
        pace = 1.0
        seen.add(FAST[1])
        out.append([READ, "o", ""])
    for key, label in CHAPTERS:
        if key not in seen and key in tail:
            seen.add(key)
            out.append([0, "m", label])
    # A prompt after a command's output: the answer is on screen, give it time to be read.
    if PROMPT.search(tail) and prompts < len(CHAPTERS) + 1:
        prompts += 1
        if prompts > 1:
            out.append([READ, "o", ""])
        tail = ""

out.append([LAST, "o", ""])
with open(dst, "w") as f:
    f.write(json.dumps({"version": 3, "term": term, "title": "llx"}) + "\n")
    f.writelines(json.dumps(e) + "\n" for e in out)
print(f"{sum(e[0] for e in out):.1f}s, {len(seen)} marks")
