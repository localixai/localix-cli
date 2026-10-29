# llx on gpt-6-luna

Score 197/199: public tests 132/132, hidden 64/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 115.9 s | 14 | 454,132 | 56,052 | 11,024 | $0.0151 |

## Failing tests

- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: lists > three pages by priority, nothing seen twice

## Changed files

```
src/app.ts | 87 ++++++++++++++++++++++++++++++++++++++++++++++++++++----------
 1 file changed, 73 insertions(+), 14 deletions(-)

new files:
src/app-types.ts
src/http.ts
src/model.ts
src/projects.ts
src/stats.ts
src/task-common.ts
src/task-create.ts
src/task-list.ts
src/tasks.ts
src/users.ts
```
