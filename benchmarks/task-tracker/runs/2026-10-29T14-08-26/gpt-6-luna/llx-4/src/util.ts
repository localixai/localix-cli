import type { State, Project, Task } from "./types";

export class Fault extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string) {
    super(message);
  }
}
export const fail = (status: number, code: string, message = code, field?: string): never => {
  throw new Fault(status, code, message, field);
};
export const valid = (ok: unknown, field: string): void => {
  if (!ok) fail(400, "validation", `Invalid ${field}`, field);
};
export const member = (p: Project, id: number): boolean =>
  id === p.ownerId || p.members.has(id);
export const projectFor = (s: State, id: number, uid: number): Project => {
  const p = s.projects.find((x) => x.id === id);
  if (!p || !member(p, uid)) return fail(404, "not_found");
  return p;
};
export const taskFor = (s: State, key: string, uid: number): Task => {
  const t = s.tasks.find((x) => x.key === key);
  if (!t || !member(s.projects.find((p) => p.id === t.projectId)!, uid)) return fail(404, "not_found");
  return t;
};
export const owner = (p: Project, uid: number): void => {
  if (p.ownerId !== uid) fail(403, "forbidden");
};
export const writer = (p: Project, uid: number): void => {
  if (uid !== p.ownerId && p.members.get(uid) !== "member") fail(403, "forbidden");
};
export const taskJson = (t: Task) => ({
  id: t.id, key: t.key, number: t.number, projectId: t.projectId, title: t.title,
  description: t.description, status: t.status, priority: t.priority, labels: t.labels,
  assigneeId: t.assigneeId, createdBy: t.createdBy, createdAt: t.createdAt,
  updatedAt: t.updatedAt, version: t.version,
});
export function makeTask(s: State, p: Project, uid: number, b: any): Task {
  const title = typeof b.title === "string" ? b.title.trim() : "";
  valid(title.length > 0 && title.length <= 200, "title");
  const description = b.description === undefined ? "" : b.description;
  valid(typeof description === "string", "description");
  const priority = b.priority ?? "normal";
  valid(["low", "normal", "high"].includes(priority), "priority");
  const labels = b.labels ?? [];
  valid(Array.isArray(labels) && labels.every((x) => typeof x === "string" &&
    x.trim().length > 0 && x.trim().length <= 30 && !/\s/.test(x.trim())), "labels");
  const assigneeId = b.assigneeId ?? null;
  valid(assigneeId === null || (Number.isInteger(assigneeId) && member(p, assigneeId)), "assigneeId");
  const now = new Date(s.now()).toISOString(), number = p.next++;
  const t: Task = { id: ++s.ids.task, key: `${p.key}-${number}`, number, projectId: p.id,
    title, description, status: "open", priority, labels: [...new Set((labels as string[])
      .map((x) => x.trim().toLowerCase()))].sort(), assigneeId, createdBy: uid, createdAt: now,
    updatedAt: now, version: 1, history: [], comments: [], dependsOn: new Set() };
  s.tasks.push(t);
  return t;
}
