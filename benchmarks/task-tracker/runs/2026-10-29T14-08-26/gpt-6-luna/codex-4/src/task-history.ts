import type { State, Task } from "./model";
import { iso } from "./model";
const clone = (value: unknown) => JSON.parse(JSON.stringify(value));
export function bump(state: State, task: Task) {
  task.version++;
  task.updatedAt = iso(state);
}
export function record(
  state: State,
  task: Task,
  userId: number,
  field: string,
  from: unknown,
  to: unknown,
) {
  task.history.push({
    at: iso(state),
    by: userId,
    field,
    from: clone(from),
    to: clone(to),
  });
}
