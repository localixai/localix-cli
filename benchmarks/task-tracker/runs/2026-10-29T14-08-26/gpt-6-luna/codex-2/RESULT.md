# codex on gpt-6-luna

Score 195/199: public tests 132/132, hidden 62/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 137.4 s | 33 | 1,314,611 | 99,891 | 13,645 | $0.0290 |

## Failing tests

- hidden: idempotency > a replay answers the task as it was created, after it changed
- hidden: limits > a refused request leaves no task behind
- hidden: projects and members > members are listed by user number, whatever order they joined in
- hidden: tasks > a change with an empty title is refused and changes nothing

## Changed files

```
src/app.ts | 85 ++++++++++++++++++++++++++++++++++++++++++++++++++------------
 1 file changed, 69 insertions(+), 16 deletions(-)

new files:
src/accounts.ts
src/context.ts
src/http.ts
src/model.ts
src/projects.ts
src/taskdata.ts
src/taskdetail.ts
```
