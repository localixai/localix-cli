# llx on gpt-6-luna

Score 195/199: public tests 132/132, hidden 62/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 191.0 s | 36 | 2,056,748 | 99,628 | 19,329 | $0.0392 |

## Failing tests

- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: limits > every authenticated route counts, writes too
- hidden: tasks > a label made of digits and dashes is taken; an empty one is refused

## Changed files

```
src/app.ts | 56 ++++++++++++++++++++++++++++++++++++++++++++------------
 1 file changed, 44 insertions(+), 12 deletions(-)

new files:
src/dependencies.ts
src/model.ts
src/projects.ts
src/response.ts
src/tasks.ts
src/users.ts
```
