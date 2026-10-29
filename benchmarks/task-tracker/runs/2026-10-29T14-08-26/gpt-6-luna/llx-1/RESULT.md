# llx on gpt-6-luna

Score 196/199: public tests 132/132, hidden 63/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 138.9 s | 26 | 1,248,924 | 86,428 | 10,612 | $0.0256 |

## Failing tests

- hidden: errors > a method /me does not take
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: limits > every authenticated route counts, writes too

## Changed files

```
src/app.ts | 19 ++++++-------------
 1 file changed, 6 insertions(+), 13 deletions(-)

new files:
src/http.ts
src/projects.ts
src/router.ts
src/state.ts
src/tasks.ts
src/users.ts
```
