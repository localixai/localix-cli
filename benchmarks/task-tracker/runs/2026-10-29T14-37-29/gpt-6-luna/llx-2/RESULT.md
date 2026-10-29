# llx on gpt-6-luna

Score 196/199: public tests 132/132, hidden 63/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 134.0 s | 26 | 1,187,764 | 81,716 | 12,700 | $0.0256 |

## Failing tests

- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: tasks > comments are numbered across tasks, and a stranger reads none

## Changed files

```
src/app.ts | 19 ++++++-------------
 1 file changed, 6 insertions(+), 13 deletions(-)

new files:
src/http.ts
src/projects.ts
src/routes.ts
src/state.ts
src/task-changes.ts
src/tasks.ts
src/users.ts
```
