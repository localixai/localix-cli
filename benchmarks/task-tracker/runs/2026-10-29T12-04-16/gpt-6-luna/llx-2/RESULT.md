# llx on gpt-6-luna

Score 195/199: public tests 132/132, hidden 62/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 181.1 s | 29 | 1,096,492 | 124,844 | 11,626 | $0.0280 |

## Failing tests

- hidden: errors > a method /me does not take
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: projects and members > only the owner removes members
- hidden: tasks > comments are numbered across tasks, and a stranger reads none

## Changed files

```
src/app.ts | 19 ++++++-------------
 1 file changed, 6 insertions(+), 13 deletions(-)

new files:
src/http.ts
src/routes-projects.ts
src/routes-tasks.ts
src/routes-users.ts
src/routes.ts
src/server.ts
src/store.ts
```
