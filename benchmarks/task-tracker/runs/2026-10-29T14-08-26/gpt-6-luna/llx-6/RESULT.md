# llx on gpt-6-luna

Score 179/199: public tests 122/132, hidden 56/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 128.0 s | 25 | 1,042,165 | 75,509 | 10,435 | $0.0224 |

## Failing tests

- dependencies > a dependency is removed, and a removed one is not found
- errors > a method a path does not take is not allowed, and Allow names those it takes
- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: errors > a method /me does not take
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: idempotency > a replay does not use up a number
- hidden: limits > every authenticated route counts, writes too
- hidden: lists > a viewer lists; a stranger finds no project
- hidden: projects and members > a stranger adding members finds no project
- hidden: projects and members > members are listed by user number, whatever order they joined in
- hidden: tasks > a label made of digits and dashes is taken; an empty one is refused
- hidden: tasks > comments are numbered across tasks, and a stranger reads none
- idempotency > a creation sent twice with one key makes one task, and the same answer
- idempotency > a key is kept for 24 hours
- idempotency > one key with another body is refused
- members > the owner adds a member, and the member lists come owner first
- rate limits > the 101st request of a minute waits for the next, as Retry-After says
- reading, changing and deleting a task > a member deletes their own tasks, the owner any; a number is never given again
- structure > no line in src/ is wider than 120 characters
- versions > a creation answers the new task's ETag

## Changed files

```
src/app.ts | 181 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++-----
 1 file changed, 167 insertions(+), 14 deletions(-)

new files:
src/http.ts
src/model.ts
src/task-items.ts
src/tasks.ts
```
