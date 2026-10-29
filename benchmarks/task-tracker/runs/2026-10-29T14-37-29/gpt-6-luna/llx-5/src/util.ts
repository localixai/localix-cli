import type { State, Task } from "./model";
export const err = (code: string, status: number, field?: string) =>
  Response.json({ error: { code, message: code.replaceAll("_", " "), ...(field ? { field } : {}) } }, { status });
export const ok = (x: unknown, status = 200, headers?: Record<string, string>) =>
  Response.json(x, { status, headers });
export function member(s: State, pid: number, uid: number): string | undefined {
  return s.members.get(pid)?.get(uid);
}
export function task(s: State, key: string): Task | undefined {
  return s.tasks.find(t => t.key === key);
}
export function clean(x: Task): any {
  const { history, comments, depends, ...out } = x;
  return out;
}
export function makeTask(s: State, p: any, uid: number, b: any, now: string): Task {
  const n = (s.sequences.get(p.id) ?? 0) + 1;
  s.sequences.set(p.id, n);
  const labels: string[] = [...new Set<string>((b.labels ?? []).map((x: string) => x.trim().toLowerCase()))].sort();
  const t: Task = {
    id: ++s.taskIds, key: `${p.key}-${n}`, number: n, projectId: p.id,
    title: b.title.trim(), description: b.description ?? "", status: "open",
    priority: b.priority ?? "normal", labels, assigneeId: b.assigneeId ?? null,
    createdBy: uid, createdAt: now, updatedAt: now, version: 1,
    history: [], comments: [], depends: new Set(),
  };
  s.tasks.push(t); return t;
}
export function validTask(b: any, s: State, pid: number): string | undefined {
  if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200) return "title";
  if (b.description !== undefined && typeof b.description !== "string") return "description";
  if (b.priority !== undefined && !["low", "normal", "high"].includes(b.priority)) return "priority";
  if (b.labels !== undefined && (!Array.isArray(b.labels) ||
      b.labels.some((x: any) =>
        typeof x !== "string" || !x.trim() || x.trim().length > 30 || /\s/.test(x.trim())
      ))) return "labels";
  if (b.assigneeId != null && !member(s, pid, b.assigneeId)) return "assigneeId";
}
