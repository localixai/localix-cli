"""Writes config.ini from two answers typed at a terminal, as an installer's questions do."""

import sys

if not sys.stdin.isatty():
    sys.exit("setup.py asks its questions at a terminal: run it in one")

name = input("Project name: ").strip()
port = input("Port [8000]: ").strip() or "8000"
if not port.isdigit():
    sys.exit(f"not a port: {port}")
with open("config.ini", "w") as config:
    config.write(f"[project]\nname = {name}\nport = {port}\n")
print(f"wrote config.ini for {name} on port {port}")
