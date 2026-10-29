# llx on gpt-6-luna

Score 48/199: public tests 39/132, hidden 9/66, types failing.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 48.0 s | 11 | 223,450 | 41,434 | 2,977 | $0.0075 |

## Failing tests

- comments > a comment does not change the task's version
- comments > an empty comment, or one over 2000 characters, is refused
- comments > anyone in the project comments, a viewer too, oldest first
- creating tasks > a description that is not a string is refused
- creating tasks > a label over 30 characters is refused
- creating tasks > a label with a space inside is refused
- creating tasks > a member creates tasks; a viewer may not; for anyone else the project is not found
- creating tasks > a priority that is not low, normal or high is refused
- creating tasks > a task takes a description, a priority and an assignee from the project
- creating tasks > a task takes its project's key and the next number
- creating tasks > a title over 200 characters is refused
- creating tasks > an assignee from outside the project is refused
- creating tasks > an empty title is refused
- creating tasks > labels are trimmed, put in lower case, made unique and sorted
- creating tasks > labels that are not a list is refused
- creating tasks > numbers count per project
- creating tasks > permission is checked before the body: a viewer's empty title is still forbidden
- creating tasks in bulk > a batch is a list of 1 to 50 tasks
- creating tasks in bulk > a batch makes its tasks in order, each as one creation would
- creating tasks in bulk > a batch of 50 is taken
- creating tasks in bulk > a viewer makes no batch
- creating tasks in bulk > one bad task refuses the whole batch, naming it by its place
- creating tasks in bulk > the first bad task is the one named
- dependencies > a deleted task leaves every dependency
- dependencies > a dependency changes neither task's version
- dependencies > a dependency given twice is a conflict
- dependencies > a dependency is removed, and a removed one is not found
- dependencies > a dependency that would close a cycle is refused, however long the cycle
- dependencies > a member adds dependencies; a viewer reads them but adds none
- dependencies > a task cannot depend on itself, on a task of another project, or on one that does not exist
- dependencies > a task depends on another, and the other blocks it
- dependencies > a task is not done while a task it depends on is not
- dependencies > both lists are in number order
- errors > a method a path does not take is not allowed, and Allow names those it takes
- errors > the token is checked before the project
- hidden: accounts > a signed-out token reaches no project
- hidden: accounts > a token signed out cannot sign out again
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
- hidden: errors > a 204 has no body
- hidden: errors > a method /me does not take
- hidden: errors > a task key of no project is not found
- hidden: errors > a transition without a body, or with a broken one, is refused
- hidden: errors > an error answer is JSON with nothing beside the error
- hidden: idempotency > a mismatch is checked within the day only
- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: idempotency > a replay does not use up a number
- hidden: idempotency > two keys make two tasks
- hidden: limits > a refused request leaves no task behind
- hidden: limits > every authenticated route counts, writes too
- hidden: lists > a deleted task leaves the list
- hidden: lists > a label and newest first
- hidden: lists > a limit of 100 is taken
- hidden: lists > a search reaches the description, and none matches nothing
- hidden: lists > a viewer lists; a stranger finds no project
- hidden: lists > one task a page, every page, to the end
- hidden: lists > priority order on other tasks
- hidden: lists > status done after a whole round
- hidden: lists > three pages by priority, nothing seen twice
- hidden: projects and members > a removed member's tasks stay, unassigned, and the member can join again
- hidden: projects and members > a stranger's member list is not found; a viewer reads it
- hidden: projects and members > members are listed by user number, whatever order they joined in
- hidden: projects and members > only the owner removes members
- hidden: stats > a project with no tasks counts zero everywhere
- hidden: stats > a tie in assignees goes by user number
- hidden: stats > done tasks are counted as done
- hidden: status and history > a description change is an entry; a refused transition is none
- hidden: status and history > a reopened task's history tells the whole round
- hidden: status and history > an unassignment is an entry to null
- hidden: status and history > labels given in another order and case are no change
- hidden: tasks > a change to an assignee from outside is refused
- hidden: tasks > a change with an empty title is refused and changes nothing
- hidden: tasks > a deleted task's comments and history are gone with it
- hidden: tasks > a label made of digits and dashes is taken; an empty one is refused
- hidden: tasks > a member's task has the member as creator, and the creation time
- hidden: tasks > a stranger's change or deletion finds no task
- hidden: tasks > a title of exactly 200 characters and a label of exactly 30 are taken
- hidden: tasks > a viewer deletes nothing, not even with a comment of theirs on it
- hidden: tasks > comments are numbered across tasks, and a stranger reads none
- hidden: tasks > tasks are numbered on, and ids go on across projects
- hidden: tasks > the owner and a member may be assignees
- hidden: versions > a stale If-Match after a transition refuses a change
- hidden: versions > a version that never was is refused
- hidden: versions > the current version, read from the ETag, goes through
- history > a change of several fields is an entry each, in the order of their names
- history > a field set to what it was makes no entry, and no new version
- history > a new task has no history
- history > a viewer reads the history
- history > each change is an entry: who, when, the field, from and to
- idempotency > a creation sent twice with one key makes one task, and the same answer
- idempotency > a key is kept for 24 hours
- idempotency > a refused creation is not kept: the same key works once the body is fixed
- idempotency > keys are each user's own
- idempotency > one key with another body is refused
- listing tasks > a cursor keeps its sort and filters' order
- listing tasks > a list holds 20 tasks unless a limit says otherwise
- listing tasks > a list is the project's tasks, oldest first, with no next page
- listing tasks > a page that ends exactly at the last task has no next page
- listing tasks > assignee=someone is refused
- listing tasks > cursor=garbage is refused
- listing tasks > filters by assignee: an id, me, or none
- listing tasks > filters by label, in any case
- listing tasks > filters by status
- listing tasks > filters combine
- listing tasks > limit=0 is refused
- listing tasks > limit=101 is refused
- listing tasks > limit=ten is refused
- listing tasks > pages by a cursor, as far as the last page
- listing tasks > searches the title and the description, in any case
- listing tasks > sort=title is refused
- listing tasks > sorts newest first, or by priority, high first, then by number
- listing tasks > status=closed is refused
- members > a removed member no longer finds the project
- members > a role that is not member or viewer, or a user who does not exist, is refused
- members > adding someone already in the project, the owner too, is a conflict
- members > only the owner adds members
- members > the owner adds a member, and the member lists come owner first
- project stats > a deleted task is not counted
- project stats > a stranger finds no stats
- project stats > counts by assignee, unassigned left out, by count then user number
- project stats > counts by status and by priority, every bucket named
- projects > a project is read by its members, and is not found for anyone else
- projects > a user lists the projects they own or belong to, oldest first
- reading, changing and deleting a task > a change answers the task with a new version and update time
- reading, changing and deleting a task > a change names no field it does not know, nor the status
- reading, changing and deleting a task > a member deletes their own tasks, the owner any; a number is never given again
- reading, changing and deleting a task > a task is read by its key, by anyone in the project
- reading, changing and deleting a task > a viewer may not change a task
- reading, changing and deleting a task > an assignee of null unassigns the task
- reading, changing and deleting a task > removing a member unassigns their tasks
- status > a member moves a task; a viewer may not
- status > a status that does not exist is refused
- status > a task goes open, in progress, done, and is reopened
- status > done cannot go to in progress, nor stay done
- status > from open to done is not a transition
- status > from open to open is not a transition
- status > work in progress can go back to open
- versions > a change made against the version read goes through
- versions > a creation answers the new task's ETag
- versions > a task's ETag is its version, quoted
- versions > a transition checks If-Match too
- versions > of two people who read version 1, the second to write is told it changed

## Changed files

```
src/app.ts | 76 +++++++++++++++++++++++++++++++++++++++++++++++++++-----------
 1 file changed, 63 insertions(+), 13 deletions(-)

new files:
src/model.ts
src/people.ts
src/util.ts
```
