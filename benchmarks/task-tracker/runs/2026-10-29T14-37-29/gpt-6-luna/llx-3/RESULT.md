# llx on gpt-6-luna

Score 197/199: public tests 132/132, hidden 64/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 102.9 s | 20 | 492,287 | 48,767 | 8,967 | $0.0138 |

## Failing tests

- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: idempotency > a replay answers the task as it was created, after it changed

## Changed files

```
src/app.ts | 20 ++++++--------------
 1 file changed, 6 insertions(+), 14 deletions(-)

new files:
src/projects.ts
src/routes.ts
src/state.ts
src/task-changes.ts
src/task-core.ts
src/task-extras.ts
src/task-list.ts
src/tasks.ts
src/users.ts
src/util.ts
```
