# llx on gpt-6-luna

Score 197/199: public tests 132/132, hidden 64/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 187.5 s | 37 | 1,544,949 | 81,653 | 12,245 | $0.0289 |

## Failing tests

- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: projects and members > members are listed by user number, whatever order they joined in

## Changed files

```
src/app.ts | 119 ++++++++++++++++++++++++++++++++++++++++++++++++++++++-------
 1 file changed, 106 insertions(+), 13 deletions(-)

new files:
src/model.ts
src/taskroutes.ts
src/util.ts
```
