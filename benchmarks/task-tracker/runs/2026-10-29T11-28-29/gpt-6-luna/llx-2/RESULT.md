# llx on gpt-6-luna

Score 194/199: public tests 132/132, hidden 61/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 148.5 s | 39 | 1,663,101 | 71,165 | 10,842 | $0.0285 |

## Failing tests

- hidden: accounts > a token signed out cannot sign out again
- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: projects and members > members are listed by user number, whatever order they joined in
- hidden: tasks > a change to an assignee from outside is refused

## Changed files

```
src/app.ts | 77 ++++++++++++++++++++++++++++++++++++++++++++++++++------------
 1 file changed, 63 insertions(+), 14 deletions(-)

new files:
src/dependencies.ts
src/model.ts
src/projects.ts
src/tasks.ts
src/users.ts
```
