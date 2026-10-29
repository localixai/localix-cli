# codex on gpt-6-luna

Score 195/199: public tests 132/132, hidden 62/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 199.6 s | 43 | 2,258,719 | 67,615 | 14,181 | $0.0358 |

## Failing tests

- hidden: batches > a bad label is named by its task's place
- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: projects and members > members are listed by user number, whatever order they joined in

## Changed files

```
src/app.ts | 63 +++++++++++++++++++++++++++++++++++++++++++++++---------------
 1 file changed, 48 insertions(+), 15 deletions(-)

new files:
src/model.ts
src/projects.ts
src/tasks.ts
src/users.ts
src/util.ts
```
