# llx on gpt-6-luna

Score 196/199: public tests 132/132, hidden 63/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 133.4 s | 26 | 813,034 | 62,954 | 9,170 | $0.0184 |

## Failing tests

- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: projects and members > members are listed by user number, whatever order they joined in

## Changed files

```
src/app.ts | 77 ++++++++++++++++++++++++++++++++++++++++++++++++++++----------
 1 file changed, 65 insertions(+), 12 deletions(-)

new files:
src/http.ts
src/model.ts
src/projects.ts
src/taskroutes.ts
src/users.ts
```
