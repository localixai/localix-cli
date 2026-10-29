# llx on gpt-6-luna

Score 195/199: public tests 132/132, hidden 62/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 139.5 s | 21 | 781,180 | 66,172 | 10,872 | $0.0192 |

## Failing tests

- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: projects and members > members are listed by user number, whatever order they joined in
- hidden: tasks > comments are numbered across tasks, and a stranger reads none

## Changed files

```
src/app.ts | 77 ++++++++++++++++++++++++++++++++++++++++++++++++++------------
 1 file changed, 63 insertions(+), 14 deletions(-)

new files:
src/model.ts
src/projects.ts
src/task-core.ts
src/task-extra.ts
src/task-list.ts
src/task-routes.ts
src/users.ts
```
