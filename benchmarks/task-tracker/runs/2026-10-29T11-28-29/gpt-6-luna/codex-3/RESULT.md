# codex on gpt-6-luna

Score 196/199: public tests 132/132, hidden 63/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 143.0 s | 23 | 998,053 | 53,541 | 10,723 | $0.0202 |

## Failing tests

- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: limits > every authenticated route counts, writes too
- hidden: projects and members > members are listed by user number, whatever order they joined in

## Changed files

```
src/app.ts | 54 ++++++++++++++++++++++++++++++++++++++++--------------
 1 file changed, 40 insertions(+), 14 deletions(-)

new files:
src/http.ts
src/model.ts
src/projects.ts
src/task-create.ts
src/tasks.ts
src/users.ts
```
