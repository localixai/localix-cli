# codex on gpt-6-luna

Score 197/199: public tests 132/132, hidden 64/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 129.4 s | 35 | 1,525,179 | 65,979 | 10,344 | $0.0264 |

## Failing tests

- hidden: projects and members > members are listed by user number, whatever order they joined in
- hidden: tasks > a label made of digits and dashes is taken; an empty one is refused

## Changed files

```
src/app.ts | 132 +++++++++++++++++++++++++++++++++++++++++++++++++++++++------
 1 file changed, 120 insertions(+), 12 deletions(-)

new files:
src/dependencies.ts
src/model.ts
src/projects.ts
src/task-utils.ts
src/tasks.ts
src/users.ts
```
