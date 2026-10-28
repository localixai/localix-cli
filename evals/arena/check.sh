#!/bin/sh
# Whether an arena task is done, in the folder given: the files the task keeps unchanged, every
# test passing with the hidden ones added, the types and the build clean. One line per failure.
# Usage: check.sh TASK DIR KEPT...
set -u
here=$(cd "$(dirname "$0")" && pwd)
task=$1
dir=$2
shift 2
fixture="$here/../fixtures/$task"
failed=0
fail() { echo "FAIL $1"; failed=1; }
cd "$dir" || exit 2
for f in "$@"; do
  cmp -s "$f" "$fixture/$f" || fail "$f was changed"
done
cp "$here/hidden/$task.test.tsx" src/hidden.test.tsx
bun test >check-test.log 2>&1 || fail "bun test: $(grep -cE '^\(fail\)' check-test.log) failing ($(grep -E '^\(fail\)' check-test.log | sed 's/^(fail) //; s/ \[.*//' | paste -sd ';' -))"
rm -f src/hidden.test.tsx
bunx tsc --noEmit >check-tsc.log 2>&1 || fail "tsc: $(grep -c 'error TS' check-tsc.log) errors"
bunx vite build >check-build.log 2>&1 || fail "vite build"
[ "$failed" = 0 ] && echo PASS
exit "$failed"
