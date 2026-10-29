# codex on gpt-6-luna

Score 197/199: public tests 132/132, hidden 64/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 204.4 s | 48 | 2,721,289 | 78,345 | 13,884 | $0.0412 |

## Failing tests

- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: tasks > a label made of digits and dashes is taken; an empty one is refused

## Changed files

```
src/app.ts | 132 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++-----
 1 file changed, 122 insertions(+), 10 deletions(-)

new files:
src/identity.ts
src/model.ts
src/projects.ts
src/stats.ts
src/task_extra.ts
src/tasks.ts
src/types.ts
src/util.ts
```
