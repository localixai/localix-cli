import type { State, Task } from "./model";

export class Fault extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string) {
    super(message);
  }
}
export function fail(status: number, code: string, message: string, field?: string): never {
  throw new Fault(status, code, message, field);
}
export function obj(value: unknown): Record<string, any> {
  if (!value || typeof value !== "object" || Array.isArray(value)) fail(400, "validation", "Expected an object");
  return value as Record<string, any>;
}
export function need(condition: unknown, field: string, message = "Invalid value"): asserts condition {
  if (!condition) fail(400, "validation", message, field);
}
export function membership(s: State, project: number, user: number): string | undefined {
  if (s.projects.find(p => p.id === project)?.ownerId === user) return "owner";
  return s.members.get(project)?.get(user);
}
export function taskByKey(s: State, key: string): Task | undefined {
  return s.tasks.find(t => t.key === key);
}
export function addTask(s: State, projectId: number, userId: number, input: Record<string, any>, now: string): Task {
  const p = s.projects.find(x => x.id === projectId)!;
  const title = typeof input.title === "string" ? input.title.trim() : "";
  need(title.length > 0 && title.length <= 200, "title");
  const description = input.description === undefined ? "" : input.description;
  need(typeof description === "string", "description");
  const priority = input.priority ?? "normal";
  need(["low", "normal", "high"].includes(priority), "priority");
  const labels = input.labels ?? [];
  need(Array.isArray(labels), "labels");
  const clean: string[] = [];
  for (const label of labels) {
    need(typeof label === "string" && /^[a-zA-Z0-9_-]{1,30}$/.test(label.trim()), "labels");
    clean.push(label.trim().toLowerCase());
  }
  const assigneeId = input.assigneeId ?? null;
  need(assigneeId === null || (Number.isInteger(assigneeId) &&
    membership(s, projectId, assigneeId) !== undefined), "assigneeId");
  const number = (s.counts.get(projectId) ?? 0) + 1;
  s.counts.set(projectId, number);
  const task: Task = {
    id: ++s.ids.task, key: `${p.key}-${number}`, number, projectId, title, description,
    status: "open", priority, labels: [...new Set(clean)].sort(), assigneeId,
    createdBy: userId, createdAt: now, updatedAt: now, version: 1,
  };
  s.tasks.push(task);
  s.history.set(task.id, []);
  s.comments.set(task.id, []);
  s.deps.set(task.id, new Set());
  return task;
}
