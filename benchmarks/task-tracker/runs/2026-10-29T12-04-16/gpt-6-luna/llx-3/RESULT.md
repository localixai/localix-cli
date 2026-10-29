# llx on gpt-6-luna

Score 194/199: public tests 132/132, hidden 61/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 181.3 s | 32 | 1,736,749 | 91,053 | 13,127 | $0.0321 |

## Failing tests

- hidden: batches > an assignee from outside refuses a batch at its place
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: limits > every authenticated route counts, writes too
- hidden: projects and members > a stranger adding members finds no project
- hidden: tasks > comments are numbered across tasks, and a stranger reads none

## Changed files

```
src/app.ts | 69 ++++++++++++++++++++++++++++++++++++++++++++++++++------------
 1 file changed, 56 insertions(+), 13 deletions(-)

new files:
src/common.ts
src/model.ts
src/projects.ts
src/task_helpers.ts
src/tasks.ts
src/users.ts
```
