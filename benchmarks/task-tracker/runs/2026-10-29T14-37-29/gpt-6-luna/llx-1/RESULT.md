# llx on gpt-6-luna

Score 188/199: public tests 126/132, hidden 61/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 142.4 s | 23 | 744,889 | 58,937 | 10,475 | $0.0180 |

## Failing tests

- dependencies > a dependency is removed, and a removed one is not found
- hidden: accounts > a token signed out cannot sign out again
- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: idempotency > a replay does not use up a number
- hidden: tasks > comments are numbered across tasks, and a stranger reads none
- idempotency > a creation sent twice with one key makes one task, and the same answer
- idempotency > a key is kept for 24 hours
- idempotency > one key with another body is refused
- reading, changing and deleting a task > a member deletes their own tasks, the owner any; a number is never given again
- structure > no line in src/ is wider than 120 characters

## Changed files

```
src/app.ts | 70 ++++++++++++++++++++++++++++++++++++++++++++++++--------------
 1 file changed, 55 insertions(+), 15 deletions(-)

new files:
src/http.ts
src/model.ts
src/projects.ts
src/tasks.ts
src/users.ts
```
