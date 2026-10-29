# llx on gpt-6-luna

Score 197/199: public tests 132/132, hidden 64/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 122.1 s | 16 | 474,894 | 56,206 | 10,753 | $0.0152 |

## Failing tests

- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: idempotency > a replay answers the task as it was created, after it changed

## Changed files

```
src/app.ts | 95 +++++++++++++++++++++++++++++++++++++++++++++++++++++---------
 1 file changed, 82 insertions(+), 13 deletions(-)

new files:
src/auth.ts
src/model.ts
src/projects.ts
src/task-routes.ts
src/util.ts
```
