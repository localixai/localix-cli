# llx on gpt-6-luna

Score 196/199: public tests 132/132, hidden 63/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 154.5 s | 34 | 1,076,175 | 101,327 | 10,445 | $0.0251 |

## Failing tests

- hidden: accounts > a token signed out cannot sign out again
- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: tasks > comments are numbered across tasks, and a stranger reads none

## Changed files

```
src/app.ts | 76 +++++++++++++++++++++++++++++++++++++++++++++++++++-----------
 1 file changed, 63 insertions(+), 13 deletions(-)

new files:
src/people.ts
src/projects.ts
src/task-core.ts
src/task-list.ts
src/task-read.ts
src/task-write.ts
src/types.ts
src/util.ts
```
