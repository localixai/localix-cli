# llx on gpt-6-luna

Score 196/199: public tests 132/132, hidden 63/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 268.5 s | 49 | 2,896,430 | 108,078 | 15,854 | $0.0466 |

## Failing tests

- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: limits > every authenticated route counts, writes too
- hidden: projects and members > members are listed by user number, whatever order they joined in

## Changed files

```
src/app.ts | 62 ++++++++++++++++++++++++++++++++++++++++++++++++--------------
 1 file changed, 48 insertions(+), 14 deletions(-)

new files:
src/model.ts
src/projects.ts
src/tasks.ts
src/users.ts
```
