# codex on gpt-6-luna

Score 198/199: public tests 132/132, hidden 65/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 112.2 s | 22 | 829,652 | 81,364 | 12,399 | $0.0218 |

## Failing tests

- hidden: idempotency > a replay answers the task as it was created, after it changed

## Changed files

```
src/app.ts | 86 ++++++++++++++++++++++++++++++++++++++++++++++++++++----------
 1 file changed, 73 insertions(+), 13 deletions(-)

new files:
src/http.ts
src/listing.ts
src/model.ts
src/projects.ts
src/tasks.ts
src/users.ts
```
