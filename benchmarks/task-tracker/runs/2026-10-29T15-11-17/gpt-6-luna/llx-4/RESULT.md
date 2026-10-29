# llx on gpt-6-luna

Score 196/199: public tests 132/132, hidden 63/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 132.7 s | 24 | 925,195 | 115,595 | 10,289 | $0.0248 |

## Failing tests

- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: limits > a refused request leaves no task behind
- hidden: limits > every authenticated route counts, writes too

## Changed files

```
src/app.ts | 54 ++++++++++++++++++++++++++++++++++++++++++------------
 1 file changed, 42 insertions(+), 12 deletions(-)

new files:
src/http.ts
src/model.ts
src/projects.ts
src/taskRoutes.ts
src/tasks.ts
src/users.ts
```
