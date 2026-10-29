# llx on gpt-6-luna

Score 190/199: public tests 132/132, hidden 57/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 254.0 s | 44 | 3,415,330 | 125,474 | 18,087 | $0.0545 |

## Failing tests

- hidden: accounts > a token signed out cannot sign out again
- hidden: batches > an assignee from outside refuses a batch at its place
- hidden: dependencies > a stranger reaches no dependency; a viewer removes none
- hidden: limits > a refused request leaves no task behind
- hidden: limits > every authenticated route counts, writes too
- hidden: lists > three pages by priority, nothing seen twice
- hidden: projects and members > members are listed by user number, whatever order they joined in
- hidden: tasks > a change to an assignee from outside is refused
- hidden: tasks > comments are numbered across tasks, and a stranger reads none

## Changed files

```
src/app.ts | 171 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++--
 1 file changed, 168 insertions(+), 3 deletions(-)

new files:
src/taskListing.ts
src/taskModel.ts
src/tasks.ts
```
