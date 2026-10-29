# llx on gpt-6-luna

Score 196/199: public tests 132/132, hidden 63/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 159.0 s | 25 | 1,035,839 | 77,247 | 13,472 | $0.0240 |

## Failing tests

- hidden: batches > an assignee from outside refuses a batch at its place
- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: limits > every authenticated route counts, writes too

## Changed files

```
src/app.ts | 27 ++++++++++++++++-----------
 1 file changed, 16 insertions(+), 11 deletions(-)

new files:
src/router.ts
src/shared.ts
src/tasks.ts
```
