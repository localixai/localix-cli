import type { State } from "./state";
import { error, invalid, json, normalizedLabels, role, taskFields } from "./http";

export function version(request: Request, current: number) {
  const match = request.headers.get("If-Match");
  return match && match !== `"${current}"`
    ? error("precondition_failed", "Version changed", 412) : undefined;
}
export function change(state: State, task: any, user: number, field: string, value: any) {
  const previous = task[field];
  if (JSON.stringify(previous) === JSON.stringify(value)) return;
  task[field] = value;
  task.version++;
  task.updatedAt = state.iso();
  state.history.get(task.id)!.push({
    at: task.updatedAt, by: user, field, from: previous, to: value,
  });
}
export function patch(c: any, task: any, user: number) {
  const { input, request, state } = c;
  if (role(state, task.projectId, user) === "viewer")
    return error("forbidden", "Forbidden", 403);
  const check = version(request, task.version);
  if (check) return check;
  const allowed = ["title", "description", "priority", "labels", "assigneeId"];
  const unknown = Object.keys(input ?? {}).find((key) => !allowed.includes(key));
  if (unknown) return invalid(unknown);
  const field = taskFields({ ...task, ...input }, user, state, task.projectId);
  if (field) return invalid(field);
  const history = state.history.get(task.id)!.length;
  for (const key of allowed.sort()) if (key in input) {
    let value = input[key];
    if (key === "title") value = value.trim();
    if (key === "labels") value = normalizedLabels(value);
    change(state, task, user, key, value);
  }
  const count = state.history.get(task.id)!.length - history;
  if (count > 1) task.version -= count - 1;
  return json(task, 200, { ETag: `"${task.version}"` });
}
