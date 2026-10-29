# codex on gpt-6-luna

Score 197/199: public tests 132/132, hidden 64/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 108.6 s | 25 | 1,044,492 | 120,204 | 10,466 | $0.0265 |

## Failing tests

- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: tasks > a label made of digits and dashes is taken; an empty one is refused

## Changed files

```
src/app.ts | 32 ++++++++++++++++++--------------
 1 file changed, 18 insertions(+), 14 deletions(-)

new files:
src/model.ts
src/projects.ts
src/router.ts
src/routes-meta.ts
src/task-admin.ts
src/task-change.ts
src/task-extra.ts
src/task-read.ts
src/tasks-create.ts
src/users.ts
```
