import type { Ctx } from "./context";
import { project } from "./context";
import { json } from "./http";
export function stats(
  c: Ctx,
  path: string,
  method: string,
): Response | undefined {
  const m = path.match(/^\/projects\/(\d+)\/stats$/);
  if (!m || method !== "GET") return undefined;
  const p = project(c, m[1]!);
  const tasks = c.s.tasks.filter((t) => t.projectId === p.id && !t.deleted);
  const count = (field: string, values: string[]) =>
    Object.fromEntries(
      values.map((v) => [
        v,
        tasks.filter((t) => (t as any)[field] === v).length,
      ]),
    );
  const assignees = new Map<number, number>();
  for (const t of tasks)
    if (t.assigneeId !== null)
      assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0) + 1);
  const byAssignee = [...assignees]
    .map(([userId, count]) => ({ userId, count }))
    .sort((a, b) => b.count - a.count || a.userId - b.userId);
  return json({
    total: tasks.length,
    byStatus: count("status", ["open", "in_progress", "done"]),
    byPriority: count("priority", ["high", "normal", "low"]),
    byAssignee,
  });
}
