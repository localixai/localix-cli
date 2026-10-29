# codex on gpt-6-luna

Score 196/199: public tests 132/132, hidden 63/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 112.4 s | 32 | 1,360,310 | 57,910 | 9,949 | $0.0238 |

## Failing tests

- hidden: batches > an assignee from outside refuses a batch at its place
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: projects and members > members are listed by user number, whatever order they joined in

## Changed files

```
src/app.ts | 116 ++++++++++++++++++++++++++++++++++++++++++++++++++++++-------
 1 file changed, 104 insertions(+), 12 deletions(-)

new files:
src/auth.ts
src/listing.ts
src/model.ts
src/projects.ts
src/task_aux.ts
src/tasks.ts
```
