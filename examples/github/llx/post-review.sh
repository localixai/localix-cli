#!/usr/bin/env bash
# Posts llx's review (review.json) on a pull request: one review with a comment per finding on a
# line of the diff, and one summary comment that each push updates. Needs GH_TOKEN, PR, BASE, HEAD.
set -euo pipefail
review=${1:-review.json}
marker='<!-- llx:review -->'

# The lines of the new side that the diff shows: a comment elsewhere fails the whole review (422).
git diff --unified=3 "$BASE...$HEAD" | awk '
  /^\+\+\+ b\// { path = substr($0, 7); next }
  /^@@/ { split($3, a, ","); line = substr(a[1], 2) + 0; next }
  /^-/ { next }
  /^[ +]/ { if (path != "") print path ":" line; line++ }
' > shown.txt

# The lines llx already commented on: a new finding there would only repeat it.
gh api "repos/{owner}/{repo}/pulls/$PR/comments" --paginate \
  --jq '.[] | select(.user.login == "github-actions[bot]" and .line != null) | "\(.path):\(.line)"' > said.txt

jq -c --rawfile shown shown.txt --rawfile said said.txt '
  ($shown | split("\n")) as $lines
  | ($said | split("\n")) as $old
  | .findings | map(select("\(.path):\(.line)" as $k | any($lines[]; . == $k) and (any($old[]; . == $k) | not)))
  | map({path, line, side: "RIGHT", body: "**\(.severity)** \(.body)"})' "$review" > inline.json

event=COMMENT
[ "$(jq -r .verdict "$review")" = request_changes ] && event=REQUEST_CHANGES
if [ "$(jq length inline.json)" -gt 0 ]; then
  jq -n --arg commit "$HEAD" --arg event "$event" --slurpfile c inline.json \
    '{commit_id: $commit, event: $event, comments: $c[0]}' |
    gh api --method POST "repos/{owner}/{repo}/pulls/$PR/reviews" --input - > /dev/null
fi

# What found no line of the diff stays in the summary.
body=$(jq -r --arg marker "$marker" --rawfile shown shown.txt '
  ($shown | split("\n")) as $lines
  | [.findings[] | select("\(.path):\(.line)" as $k | any($lines[]; . == $k) | not)
     | "- `\(.path):\(.line)` **\(.severity)** \(.body)"] as $rest
  | "\($marker)\n### llx review: \(.verdict | gsub("_"; " "))\n\n\(.summary)\n"
    + (if ($rest | length) > 0 then "\n" + ($rest | join("\n")) + "\n" else "" end)
    + "\n<sub>\(.findings | length) findings · commit \(env.HEAD[0:7])</sub>"' "$review")

id=$(gh api "repos/{owner}/{repo}/issues/$PR/comments" --paginate \
  --jq ".[] | select(.user.login == \"github-actions[bot]\" and (.body | startswith(\"$marker\"))) | .id" | head -n1)
if [ -n "$id" ]; then
  gh api --method PATCH "repos/{owner}/{repo}/issues/comments/$id" -f body="$body" > /dev/null
else
  gh api --method POST "repos/{owner}/{repo}/issues/$PR/comments" -f body="$body" > /dev/null
fi
