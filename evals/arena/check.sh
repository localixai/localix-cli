#!/bin/sh
# Whether the reference task is done, in the folder given: the visible tests unchanged, every
# test passing with the hidden ones added, the types and the build clean. One line per failure.
set -u
here=$(cd "$(dirname "$0")" && pwd)
dir=$1
fixture="$here/../fixtures/ledger-web"
failed=0
fail() { echo "FAIL $1"; failed=1; }
cd "$dir" || exit 2
for f in src/lib/csv.test.ts src/lib/report.test.ts; do
  cmp -s "$f" "$fixture/$f" || fail "$f was changed"
done
cp "$here/hidden/ledger-web.test.tsx" src/hidden.test.tsx
bun test >check-test.log 2>&1 || fail "bun test: $(grep -cE '^\(fail\)' check-test.log) failing ($(grep -E '^\(fail\)' check-test.log | sed 's/^(fail) //; s/ \[.*//' | paste -sd ';' -))"
rm -f src/hidden.test.tsx
bunx tsc --noEmit >check-tsc.log 2>&1 || fail "tsc: $(grep -c 'error TS' check-tsc.log) errors"
bunx vite build >check-build.log 2>&1 || fail "vite build"
[ "$failed" = 0 ] && echo PASS
exit "$failed"
