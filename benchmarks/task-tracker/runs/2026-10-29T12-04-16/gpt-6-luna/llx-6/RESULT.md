# llx on gpt-6-luna

Score 195/199: public tests 132/132, hidden 62/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 96.5 s | 15 | 404,830 | 47,710 | 10,364 | $0.0135 |

## Failing tests

- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: projects and members > members are listed by user number, whatever order they joined in
- hidden: tasks > a label made of digits and dashes is taken; an empty one is refused
- hidden: tasks > comments are numbered across tasks, and a stranger reads none

## Changed files

```
src/app.ts | 83 +++++++++++++++++++++++++++++++++++++++++++++++++++-----------
 1 file changed, 69 insertions(+), 14 deletions(-)

new files:
src/http.ts
src/model.ts
src/people.ts
src/projects.ts
src/stats.ts
src/task-core.ts
src/task-delete.ts
src/task-extra.ts
src/task-list.ts
```
