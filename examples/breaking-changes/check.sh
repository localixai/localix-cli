#!/bin/sh
# What a change does to an HTTP API's clients, read by llx: the spec's structure, and what no
# structural diff can see (descriptions, defaults, the code that serves it).
#
# Run in the repository, after `git fetch`: check.sh [SPEC] [BASE]
#   SPEC  the spec's directory or file (src/spec)
#   BASE  what the change is compared with (origin/main)
# Writes breaking-changes.json (LLX_OUT to change it) and prints a line per change.
# Exit: 0 nothing breaks, 1 something breaks, 2 llx could not finish.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
spec=${1:-src/spec}
base=${2:-origin/main}
out=${LLX_OUT:-breaking-changes.json}
prompt=$(sed -e "s|{SPEC}|$spec|g" -e "s|{BASE}|$base|g" "$here/prompt.md")

# Read only: llx's own `read`, and commands that change nothing. Anything else is refused, as no
# one is there to ask. stdin is closed: with the prompt on the command line, an open stdin (a CI
# runner's, `ssh` without -n) would be read to its end first.
llx exec --deny edit \
  --allow 'bash(git diff *)' --allow 'bash(git show *)' --allow 'bash(git log *)' \
  --allow 'bash(git status *)' --allow 'bash(ls *)' --allow 'bash(grep *)' \
  --max-turns 30 --timeout 300 --schema "@$here/schema.json" -o "$out" "$prompt" </dev/null >/dev/null ||
  { echo "llx could not finish: exit $?" >&2; exit 2; }

jq -r '.changes[] | "\(if .breaking then "BREAKS" else "safe  " end)  \(if .kind == "semantic" then "semantic  " else "structural" end)  \(.endpoint): \(.change)"' "$out"
[ "$(jq -r .breaking "$out")" = false ] || exit 1
