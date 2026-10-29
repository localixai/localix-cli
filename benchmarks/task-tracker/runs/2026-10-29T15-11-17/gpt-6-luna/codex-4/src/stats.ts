import type { State } from "./model";
import { fail, memberRole } from "./util";

export function stats(s: State, path: string, uid: number) {
  const id = Number(path.split("/")[2]);
  const p = s.projects.find((x) => x.id === id);
  if (!p || !memberRole(s, p, uid)) fail(404, "not_found", "project not found");
  const tasks = s.tasks.filter((t) => t.projectId === id);
  const tally = (field: "status" | "priority", keys: string[]) =>
    Object.fromEntries(keys.map((key) => [key, tasks.filter((t) => t[field] === key).length]));
  const groups = new Map<number, number>();
  for (const t of tasks) if (t.assigneeId !== null) groups.set(t.assigneeId, (groups.get(t.assigneeId) || 0) + 1);
  const byAssignee = [...groups]
    .map(([userId, count]) => ({ userId, count }))
    .sort((a, b) => b.count - a.count || a.userId - b.userId);
  return {
    status: 200,
    body: {
      total: tasks.length,
      byStatus: tally("status", ["open", "in_progress", "done"]),
      byPriority: tally("priority", ["high", "normal", "low"]),
      byAssignee,
    },
  };
}
