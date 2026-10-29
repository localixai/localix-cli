import { State, Task, bad, conflict, iso, memberRole } from "./model";
export function validateTask(s: State, p: any, b: any): string | null {
  if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200) return "title";
  if (b.description !== undefined && typeof b.description !== "string") return "description";
  if (b.priority !== undefined && !["low", "normal", "high"].includes(b.priority)) return "priority";
  if (b.labels !== undefined && (!Array.isArray(b.labels) ||
      b.labels.some((x: any) => typeof x !== "string" || !x.trim() ||
        x.trim().length > 30 || /\s/.test(x.trim())))) return "labels";
  if (b.assigneeId !== undefined && b.assigneeId !== null &&
      (!memberRole(s, p, b.assigneeId) || !Number.isInteger(b.assigneeId))) return "assigneeId";
  return null;
}
export function labels(xs: string[] = []) { return [...new Set(xs.map((x) => x.trim().toLowerCase()))].sort(); }
export function createTask(s: State, p: any, uid: number, b: any): Task {
  const number = (s.taskCounters.get(p.id) || 0) + 1;
  s.taskCounters.set(p.id, number);
  const at = iso(s);
  const task: Task = { id: ++s.tid, key: `${p.key}-${number}`, number, projectId: p.id,
    title: b.title.trim(), description: b.description ?? "", status: "open", priority: b.priority ?? "normal",
    labels: labels(b.labels), assigneeId: b.assigneeId ?? null, createdBy: uid, createdAt: at,
    updatedAt: at, version: 1 };
  s.tasks.push(task); s.history.set(task.id, []); s.comments.set(task.id, []); s.deps.set(task.id, new Set());
  return task;
}
export function writeHistory(s: State, t: Task, uid: number, field: string, from: any, to: any) {
  s.history.get(t.id)!.push({ at: iso(s), by: uid, field, from, to });
}
