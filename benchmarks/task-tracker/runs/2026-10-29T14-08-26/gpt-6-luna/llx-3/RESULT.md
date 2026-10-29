# llx on gpt-6-luna

Score 197/199: public tests 132/132, hidden 64/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 164.9 s | 23 | 846,553 | 116,825 | 13,636 | $0.0258 |

## Failing tests

- hidden: limits > a refused request leaves no task behind
- hidden: projects and members > members are listed by user number, whatever order they joined in

## Changed files

```
src/app.ts | 48 ++++++++++++++++++++++++++++++++++--------------
 1 file changed, 34 insertions(+), 14 deletions(-)

new files:
src/dependencies.ts
src/http.ts
src/model.ts
src/routes.ts
src/task-helpers.ts
```
