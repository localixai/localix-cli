import { type Ctx, auth, ok } from "./http";
export function stats(c: Ctx, path: string, method: string): Response | null {
  const m = path.match(/^\/projects\/(\d+)\/stats$/);
  if (!m || method !== "GET") return null;
  const id = Number(m[1]); auth(c, undefined, id);
  const tasks = c.s.tasks.filter((t) => t.projectId === id);
  const count = (field: string, values: string[]) => Object.fromEntries(values.map((v) =>
    [v, tasks.filter((t) => (t as any)[field] === v).length]));
  const assigned = new Map<number, number>();
  for (const t of tasks) if (t.assigneeId !== null) assigned.set(t.assigneeId, (assigned.get(t.assigneeId) ?? 0) + 1);
  const byAssignee = [...assigned].map(([userId, count]) => ({ userId, count }))
    .sort((a, b) => b.count - a.count || a.userId - b.userId);
  return ok({ total: tasks.length, byStatus: count("status", ["open", "in_progress", "done"]),
    byPriority: count("priority", ["high", "normal", "low"]), byAssignee });
}
