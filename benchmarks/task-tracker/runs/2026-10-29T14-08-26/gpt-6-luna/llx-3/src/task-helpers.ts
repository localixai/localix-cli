import { type Project, type State, type Task } from "./model";
import { fail, validate } from "./http";
export type Ctx = { req: Request; url: URL; s: State; now: () => number; token?: string; userId?: number };
export const stamp = (c: Ctx) => new Date(c.now()).toISOString();
export const user = (c: Ctx) => {
  if (!c.userId) fail(401, "unauthorized", "Sign in required");
  return c.s.users.find(x => x.id === c.userId)!;
};
export const proj = (c: Ctx, id: number, member = true) => {
  const p = c.s.projects.find(x => x.id === id);
  if (!p || (member && !p.members.has(user(c).id))) fail(404, "not_found", "Not found");
  return p!;
};
export const task = (c: Ctx, key: string) => {
  const t = c.s.tasks.find(x => x.key === key);
  if (!t) fail(404, "not_found", "Not found");
  proj(c, t!.projectId);
  return t!;
};
export const owner = (p: Project, u: number) => p.ownerId === u;
export const editor = (p: Project, u: number) => owner(p, u) || p.members.get(u) === "member";
export function validTask(x: any, p: Project, c: Ctx, existing?: Task) {
  const title = x.title === undefined && existing ? existing.title : x.title;
  validate(typeof title === "string" && title.trim().length > 0 && title.trim().length <= 200, "title");
  const desc = x.description === undefined ? existing?.description ?? "" : x.description;
  validate(typeof desc === "string", "description");
  const priority = x.priority ?? existing?.priority ?? "normal";
  validate(["low", "normal", "high"].includes(priority), "priority");
  const labels = x.labels ?? existing?.labels ?? [];
  validate(Array.isArray(labels) && labels.every((l: any) => typeof l === "string" &&
    l.trim().length > 0 && l.trim().length <= 30 && !/\s/.test(l.trim())), "labels");
  const assignee = x.assigneeId === undefined ? existing?.assigneeId ?? null : x.assigneeId;
  validate(assignee === null || (Number.isInteger(assignee) && p.members.has(assignee)), "assigneeId");
  return { title: title.trim(), description: desc, priority, assigneeId: assignee,
    labels: [...new Set(labels.map((l: string) => l.trim().toLowerCase()))].sort() as string[] };
}
export function createTask(c: Ctx, p: Project, x: any) {
  const v = validTask(x, p, c), n = p.next++;
  const t: Task = { ...v, id: ++c.s.tid, key: `${p.key}-${n}`, number: n, projectId: p.id,
    status: "open", createdBy: user(c).id, createdAt: stamp(c), updatedAt: stamp(c),
    version: 1, comments: [], history: [], deps: new Set() };
  c.s.tasks.push(t); return t;
}
export function change(c: Ctx, t: Task, x: any) {
  const keys = Object.keys(x), allowedKeys = ["title", "description", "priority", "labels", "assigneeId"];
  for (const k of keys) validate(allowedKeys.includes(k), k);
  const p = c.s.projects.find(z => z.id === t.projectId)!;
  const v = validTask(x, p, c, t), at = stamp(c), before: any = { ...t };
  for (const k of keys) (t as any)[k] = (v as any)[k];
  const changed = keys.filter(k => JSON.stringify(before[k]) !== JSON.stringify((t as any)[k]));
  if (changed.length) {
    t.version++; t.updatedAt = at;
    for (const k of changed.sort()) t.history.push({ at, by: user(c).id, field: k,
      from: before[k], to: (t as any)[k] });
  }
}
export const etag = (t: Task) => ({ ETag: `"${t.version}"` });
export function match(c: Ctx, t: Task) {
  const tag = c.req.headers.get("If-Match");
  if (tag && tag !== `"${t.version}"`) fail(412, "precondition_failed", "Task has changed");
}
