# codex on gpt-6-luna

Score 192/199: public tests 132/132, hidden 59/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 108.6 s | 27 | 1,102,173 | 96,605 | 10,507 | $0.0250 |

## Failing tests

- hidden: accounts > a token signed out cannot sign out again
- hidden: batches > a bad label is named by its task's place
- hidden: batches > an assignee from outside refuses a batch at its place
- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: projects and members > members are listed by user number, whatever order they joined in
- hidden: tasks > comments are numbered across tasks, and a stranger reads none

## Changed files

```
src/app.ts | 116 +++++++++++++++++++++++++++++++++++++++++++++++++++++++------
 1 file changed, 105 insertions(+), 11 deletions(-)

new files:
src/dependencies.ts
src/model.ts
src/projects.ts
src/task-core.ts
src/tasks.ts
src/users.ts
```
