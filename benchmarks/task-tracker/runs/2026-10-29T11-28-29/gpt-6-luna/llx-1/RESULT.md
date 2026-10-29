# llx on gpt-6-luna

Score 146/199: public tests 99/132, hidden 47/66, types failing.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 97.9 s | 14 | 403,637 | 46,773 | 7,879 | $0.0122 |

## Failing tests

- creating tasks > a member creates tasks; a viewer may not; for anyone else the project is not found
- creating tasks > permission is checked before the body: a viewer's empty title is still forbidden
- creating tasks in bulk > a batch is a list of 1 to 50 tasks
- creating tasks in bulk > a batch makes its tasks in order, each as one creation would
- creating tasks in bulk > a batch of 50 is taken
- creating tasks in bulk > a viewer makes no batch
- creating tasks in bulk > one bad task refuses the whole batch, naming it by its place
- creating tasks in bulk > the first bad task is the one named
- dependencies > a deleted task leaves every dependency
- dependencies > a dependency given twice is a conflict
- dependencies > a dependency is removed, and a removed one is not found
- dependencies > a dependency that would close a cycle is refused, however long the cycle
- dependencies > a member adds dependencies; a viewer reads them but adds none
- dependencies > a task cannot depend on itself, on a task of another project, or on one that does not exist
- dependencies > a task depends on another, and the other blocks it
- dependencies > a task is not done while a task it depends on is not
- dependencies > both lists are in number order
- errors > a path that does not exist is not found
- hidden: batches > a bad label is named by its task's place
- hidden: batches > a batch goes on from the project's last number, and its tasks have no history
- hidden: batches > a batch that is not a list is refused
- hidden: batches > an assignee from outside refuses a batch at its place
- hidden: dependencies > a cycle through four tasks is refused, a diamond is not
- hidden: dependencies > a dependency named by something that is not a string is refused
- hidden: dependencies > a refused cycle adds nothing
- hidden: dependencies > a reopened dependency blocks again
- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: dependencies > a task with two dependencies waits for both
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: limits > a refused request leaves no task behind
- hidden: limits > every authenticated route counts, writes too
- hidden: lists > a deleted task leaves the list
- hidden: lists > a label and newest first
- hidden: lists > one task a page, every page, to the end
- hidden: lists > priority order on other tasks
- hidden: lists > three pages by priority, nothing seen twice
- hidden: projects and members > a removed member's tasks stay, unassigned, and the member can join again
- listing tasks > a cursor keeps its sort and filters' order
- listing tasks > assignee=someone is refused
- listing tasks > cursor=garbage is refused
- listing tasks > filters by assignee: an id, me, or none
- listing tasks > limit=0 is refused
- listing tasks > limit=101 is refused
- listing tasks > limit=ten is refused
- listing tasks > sort=title is refused
- listing tasks > sorts newest first, or by priority, high first, then by number
- listing tasks > status=closed is refused
- rate limits > Retry-After rounds a part of a second up
- rate limits > a token's requests say the limit and what is left of it this minute
- rate limits > each token has a limit of its own
- rate limits > the 101st request of a minute waits for the next, as Retry-After says
- structure > no line in src/ is wider than 120 characters

## Changed files

```
src/app.ts | 77 ++++++++++++++++++++++++++++++++++++++++++++++++++++----------
 1 file changed, 65 insertions(+), 12 deletions(-)

new files:
src/accounts.ts
src/common.ts
src/model.ts
src/projects.ts
src/tasks.ts
```
