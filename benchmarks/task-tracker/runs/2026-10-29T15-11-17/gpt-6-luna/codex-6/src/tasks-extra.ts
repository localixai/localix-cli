import { fail, stamp, validation } from "./http";
import type { Context, Project, Task, User } from "./model";
import { byKey, clean, createTask, plainTask } from "./taskops";
import { memberRole, writable } from "./projects";
export function bulk(c: Context, p: Project, user: User, body: Record<string, any>) {
  if (!Array.isArray(body.tasks) || body.tasks.length < 1 || body.tasks.length > 50) validation("tasks");
  const values = body.tasks.map((item: unknown, index: number) => {
    if (!item || typeof item !== "object" || Array.isArray(item)) validation(`tasks[${index}]`);
    try { return clean(item as Record<string, any>, c, p); }
    catch (e) {
      if (e instanceof Error && "field" in e) {
        const error = e as Error & { field: string };
        validation(`tasks[${index}].${error.field}`);
      }
      throw e;
    }
  });
  return { items: values.map((v: any) => plainTask(createTask(c, p, user, v))) };
}
export function list(c: Context, p: Project, url: URL, userId: number) {
  const q = url.searchParams;
  const status = q.get("status"); const sort = q.get("sort") ?? "number";
  if (status && !["open", "in_progress", "done"].includes(status)) validation("status");
  if (!["number", "-created", "priority"].includes(sort)) validation("sort");
  const limitText = q.get("limit"); const limit = limitText === null ? 20 : Number(limitText);
  if (!Number.isInteger(limit) || limit < 1 || limit > 100) validation("limit");
  const assignee = q.get("assignee");
  if (assignee && assignee !== "me" && assignee !== "none" && !/^\d+$/.test(assignee)) validation("assignee");
  let items = c.s.tasks.filter((t) => t.projectId === p.id);
  if (status) items = items.filter((t) => t.status === status);
  if (assignee === "none") items = items.filter((t) => t.assigneeId === null);
  else if (assignee === "me") items = items.filter((t) => t.assigneeId === userId);
  else if (assignee) items = items.filter((t) => t.assigneeId === Number(assignee));
  const label = q.get("label")?.toLowerCase(); if (label) items = items.filter((t) => t.labels.includes(label));
  const search = q.get("q")?.toLowerCase();
  if (search) items = items.filter((t) => `${t.title} ${t.description}`.toLowerCase().includes(search));
  const priority = { high: 0, normal: 1, low: 2 } as Record<string, number>;
  if (sort === "-created") items.sort((a, b) => b.number - a.number);
  else if (sort === "priority") {
    items.sort((a, b) => priority[a.priority]! - priority[b.priority]! || a.number - b.number);
  }
  else items.sort((a, b) => a.number - b.number);
  const cursor = q.get("cursor");
  if (cursor) {
    try {
      const decoded = JSON.parse(atob(cursor));
      const index = items.findIndex((t) => t.key === decoded.key);
      if (index < 0) validation("cursor");
      items = items.slice(index + 1);
    }
    catch { validation("cursor"); }
  }
  const page = items.slice(0, limit); const more = items.length > limit;
  const nextCursor = more ? btoa(JSON.stringify({ key: page[page.length - 1]!.key })) : null;
  return { items: page.map(plainTask), nextCursor };
}
export function stats(c: Context, p: Project) {
  const tasks = c.s.tasks.filter((t) => t.projectId === p.id);
  const byStatus = { open: 0, in_progress: 0, done: 0 };
  const byPriority = { high: 0, normal: 0, low: 0 };
  const assigned = new Map<number, number>();
  for (const t of tasks) { (byStatus as any)[t.status]++; (byPriority as any)[t.priority]++;
    if (t.assigneeId !== null) assigned.set(t.assigneeId, (assigned.get(t.assigneeId) ?? 0) + 1); }
  return { total: tasks.length, byStatus, byPriority,
    byAssignee: [...assigned].map(([userId, count]) => ({ userId, count }))
      .sort((a, b) => b.count - a.count || a.userId - b.userId) };
}
export function comment(c: Context, t: Task, user: User, body: Record<string, any>) {
  const text = typeof body.body === "string" ? body.body.trim() : "";
  if (!text || text.length > 2000) validation("body");
  const item = { id: ++c.s.ids.comment, authorId: user.id, body: text, createdAt: stamp(c.now) };
  c.s.comments.get(t.id)!.push(item); return item;
}
export function dependencies(c: Context, t: Task) {
  const ids = c.s.deps.get(t.id)!;
  const keys = (values: number[]) => values.map((id) => c.s.tasks.find((x) => x.id === id)!)
    .filter(Boolean).sort((a, b) => a.number - b.number).map((x) => x.key);
  return { key: t.key, dependsOn: keys([...ids]),
    blocks: keys(c.s.tasks.filter((x) => c.s.deps.get(x.id)?.has(t.id)).map((x) => x.id)) };
}
export function addDependency(c: Context, t: Task, body: Record<string, any>) {
  if (typeof body.dependsOn !== "string") validation("dependsOn");
  const other = c.s.tasks.find((x) => x.key === body.dependsOn);
  if (!other || other.projectId !== t.projectId || other.id === t.id) validation("dependsOn");
  const set = c.s.deps.get(t.id)!;
  if (set.has(other.id)) fail(409, "conflict", "Dependency already exists");
  const reaches = (from: number, target: number, seen = new Set<number>()): boolean => {
    if (from === target) return true;
    if (seen.has(from)) return false; seen.add(from);
    return [...(c.s.deps.get(from) ?? [])].some((id) => reaches(id, target, seen));
  };
  if (reaches(other.id, t.id)) fail(409, "dependency_cycle", "Dependency would create a cycle");
  set.add(other.id); return dependencies(c, t);
}
export function deleteTask(c: Context, t: Task) {
  c.s.tasks = c.s.tasks.filter((x) => x.id !== t.id);
  c.s.deps.delete(t.id); for (const deps of c.s.deps.values()) deps.delete(t.id);
}
export function canRead(c: Context, p: Project, userId: number) { return !!memberRole(c, p, userId); }
