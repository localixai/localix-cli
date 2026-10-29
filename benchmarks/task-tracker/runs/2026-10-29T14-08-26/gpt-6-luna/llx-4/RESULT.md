# llx on gpt-6-luna

Score 196/199: public tests 132/132, hidden 63/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 120.1 s | 22 | 760,963 | 55,683 | 11,078 | $0.0182 |

## Failing tests

- hidden: accounts > a token signed out cannot sign out again
- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: lists > three pages by priority, nothing seen twice

## Changed files

```
src/app.ts | 89 ++++++++++++++++++++++++++++++++++++++++++++++++++++----------
 1 file changed, 75 insertions(+), 14 deletions(-)

new files:
src/projects.ts
src/task-list.ts
src/tasks.ts
src/types.ts
src/users.ts
src/util.ts
```
