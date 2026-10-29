import { check, fail } from "./http";
import { memberRole, pair, type Project, type State, type Task } from "./model";

export function cleanTask(body: any, user: number, project: Project, state: State) {
  check(typeof body.title === "string" && body.title.trim().length > 0 &&
    body.title.trim().length <= 200, "title");
  const description = body.description === undefined ? "" : body.description;
  check(typeof description === "string", "description");
  const priority = body.priority ?? "normal";
  check(["low", "normal", "high"].includes(priority), "priority");
  const labels = body.labels ?? [];
  check(Array.isArray(labels) && labels.every((x: any) =>
    typeof x === "string" && x.trim().length <= 30 && /^[\w-]*$/.test(x.trim())), "labels");
  const assigneeId = body.assigneeId ?? null;
  check(assigneeId === null || (typeof assigneeId === "number" &&
    memberRole(state, project, assigneeId) !== undefined), "assigneeId");
  return { title: body.title.trim(), description, priority,
    labels: [...new Set(labels.map((x: string) => x.trim().toLowerCase()))].sort(), assigneeId, user };
}
export function createTask(state: State, project: Project, fields: any, now: string): Task {
  const number = state.tasks.filter(t => t.projectId === project.id)
    .reduce((n, t) => Math.max(n, t.number), 0) + 1;
  const task: Task = {
    id: state.tasks.length + 1, key: `${project.key}-${number}`, number,
    projectId: project.id, title: fields.title, description: fields.description,
    status: "open", priority: fields.priority, labels: fields.labels,
    assigneeId: fields.assigneeId, createdBy: fields.user, createdAt: now, updatedAt: now, version: 1,
  };
  state.tasks.push(task);
  return task!;
}
export function historyChange(state: State, task: Task, user: number, field: string, from: any, to: any, at: string) {
  const key = String(task.id);
  const list = state.history.get(key) ?? [];
  list.push({ at, by: user, field, from, to });
  state.history.set(key, list);
}
export function memberCanRead(state: State, project: Project, user: number) {
  return memberRole(state, project, user) !== undefined;
}
export function taskFor(state: State, key: string): Task {
  const task = state.tasks.find(t => t.key === key);
  if (!task) fail(404, "not_found", "not found");
  return task!;
}
