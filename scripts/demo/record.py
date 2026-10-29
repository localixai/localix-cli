#!/usr/bin/env python3
"""Record the demo: llx driven at a real terminal, as a person types, into an asciinema v3 cast
with chapter markers (site/public/demo-llx.cast; the README's gif is made from it with agg).

  scripts/demo/record.py [OUT.cast]        # needs an OpenRouter key and `llx` on the PATH

It runs in a copy of evals/fixtures/fix-test under a temporary home-like directory shown as
demo/stats, through zsh with a plain prompt, 100x30. Each step waits for what the screen must show
before the next key, so the timing is the model's, not a guess.
"""
import json
import os
import pty
import random
import re
import select
import shutil
import struct
import sys
import termios
import time
import fcntl

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
COLS, ROWS = 100, 30
ESCAPES = re.compile(rb"\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07]*\x07|\x1b[()][B0]|\x1b[=>]")

GOAL = "The median test fails. Fix stats.py and run python3 -m unittest."
SCHEMA = '{"type":"object","properties":{"passing":{"type":"integer"}},"required":["passing"]}'
EXEC = f"llx exec --mode plan --schema '{SCHEMA}' 'How many tests pass? Run them.'; echo exit=$?"
DECIDE = "git diff | llx decide 'Does this change break the public API?'"


class Terminal:
    def __init__(self, cwd, home):
        self.events = []  # (gap, kind, data)
        self.last = time.monotonic()
        self.seen = b""
        pid, fd = pty.fork()
        if pid == 0:
            os.chdir(cwd)
            path = os.path.join(ROOT, "target/release") + ":" + os.environ["PATH"]
            env = dict(os.environ, TERM="xterm-256color", LLX_NO_DAEMON="1", ZDOTDIR=home, PATH=path)
            os.execvpe("zsh", ["zsh", "-i"], env)
        self.fd = fd
        fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

    def record(self, kind, data):
        now = time.monotonic()
        self.events.append((round(now - self.last, 3), kind, data))
        self.last = now

    def drain(self, seconds):
        end = time.monotonic() + seconds
        while True:
            left = end - time.monotonic()
            if left <= 0:
                return
            ready, _, _ = select.select([self.fd], [], [], min(left, 0.05))
            if ready:
                try:
                    chunk = os.read(self.fd, 65536)
                except OSError:
                    return
                if chunk:
                    self.record("o", chunk.decode("utf8", "replace"))
                    self.seen += chunk

    def screen(self):
        return ESCAPES.sub(b"", self.seen).decode("utf8", "replace")

    def expect(self, pattern, timeout=120):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            self.drain(0.2)
            if re.search(pattern, self.screen()):
                return
        tail = "\n".join(l for l in re.split(r"[\r\n]+", self.screen()) if l.strip())[-1500:]
        raise SystemExit(f"never saw {pattern!r}; the screen ended with:\n{tail}")

    def forget(self):
        self.seen = b""

    def key(self, data):
        os.write(self.fd, data.encode())
        self.drain(0.08)

    def type(self, text, cps=14.0):
        for ch in text:
            os.write(self.fd, ch.encode())
            self.drain(random.uniform(0.6, 1.6) / cps)

    def enter(self):
        self.key("\r")

    def marker(self, label):
        self.record("m", label)

    def pause(self, seconds):
        self.drain(seconds)


def scenario(t):
    t.expect(r"demo/stats \$", 10)
    t.pause(0.8)

    t.marker("model")
    t.type("llx")
    t.enter()
    t.expect(r"Type a task", 20)
    t.pause(1.0)
    t.type("/model")
    t.enter()
    t.expect(r"Search:", 10)
    t.pause(0.5)
    t.type("luna")
    t.pause(0.9)
    t.forget()
    t.enter()
    t.expect(r"Now openrouter", 10)
    t.pause(1.2)

    t.marker("fix the tests")
    t.forget()
    t.key("\x1b[Z")
    t.pause(0.7)
    if "bypass mode" not in t.screen():
        t.key("\x1b[Z")
        t.expect(r"bypass mode", 5)
    t.pause(0.8)
    t.type(GOAL)
    t.pause(0.6)
    t.forget()
    t.enter()
    t.expect(r"Worked for", 240)  # the rule under a task's end: how long, how much
    t.pause(3.0)

    t.marker("diff")
    t.type("/diff")
    t.enter()
    t.pause(4.0)

    t.marker("in CI")
    t.type("/exit")
    t.enter()
    t.expect(r"demo/stats \$", 10)
    t.pause(0.8)
    t.type(EXEC)
    t.forget()
    t.enter()
    t.expect(r"exit=\d", 240)
    t.pause(2.5)

    t.marker("decide")
    t.type(DECIDE)
    t.forget()
    t.enter()
    t.expect(r"\n(yes|no)\r?\n", 60)
    t.pause(3.0)


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "site/public/demo-llx.cast")
    # A fixed, short path: the trace names the directory a command ran in.
    home = "/tmp/demo"
    shutil.rmtree(home, ignore_errors=True)
    os.makedirs(home)
    work = os.path.join(home, "stats")
    shutil.copytree(os.path.join(ROOT, "evals/fixtures/fix-test"), work)
    os.system(f"cd {work} && git init -q && git add -A && "
              f"git -c user.name=demo -c user.email=demo@example.com commit -qm stats")
    with open(os.path.join(home, ".zshrc"), "w") as rc:
        rc.write("unsetopt PROMPT_SP; setopt NO_BEEP\nPROMPT='%F{242}%2~%f $ '\nclear\n")
    t = Terminal(work, home)
    try:
        scenario(t)
    finally:
        try:
            os.write(t.fd, b"\x04")
        except OSError:
            pass
        t.drain(0.5)
        with open(out, "w") as cast:
            cast.write(json.dumps({"version": 3, "term": {"cols": COLS, "rows": ROWS}, "title": "llx"}) + "\n")
            for gap, kind, data in t.events:
                cast.write(json.dumps([gap, kind, data], ensure_ascii=False) + "\n")
        shutil.rmtree(home, ignore_errors=True)
        total = sum(gap for gap, _, _ in t.events)
        print(f"{out}: {len(t.events)} events, {total:.1f} s")


if __name__ == "__main__":
    main()
