# llx on gpt-6-luna

Score 196/199: public tests 132/132, hidden 63/66, types ok.

| Time | Requests | Input | Read afresh | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| 119.3 s | 17 | 515,051 | 53,739 | 12,165 | $0.0161 |

## Failing tests

- hidden: limits > a refused request leaves no task behind
- hidden: limits > every authenticated route counts, writes too
- hidden: tasks > comments are numbered across tasks, and a stranger reads none

## Changed files

```
src/app.ts | 93 ++++++++++++++++++++++++++++++++++++++++++++++++++++++--------
 1 file changed, 81 insertions(+), 12 deletions(-)

new files:
src/http.ts
src/listing.ts
src/model.ts
src/projects.ts
src/tasks.ts
src/users.ts
```
