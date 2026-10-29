import type { State } from "./model";
import { visibleProject } from "./model";
import { auth, fail, json } from "./http";
export function stats(req: Request, path: string[], s: State): Response | undefined {
  if (path[0] !== "projects" || path[2] !== "stats" || path.length !== 3) return undefined;
  const u = auth(req, s), id = Number(path[1]);
  const p = s.projects.find((x) => x.id === id);
  if (!p || !visibleProject(s, p, u)) fail(404, "not_found", "Project not found");
  const tasks = s.tasks.filter((t) => !t.deleted && t.projectId === id);
  const byStatus = { open: 0, in_progress: 0, done: 0 };
  const byPriority = { high: 0, normal: 0, low: 0 };
  const counts = new Map<number, number>();
  for (const t of tasks) {
    byStatus[t.status as keyof typeof byStatus]++;
    byPriority[t.priority as keyof typeof byPriority]++;
    if (t.assigneeId !== null) counts.set(t.assigneeId, (counts.get(t.assigneeId) ?? 0) + 1);
  }
  const byAssignee = [...counts].map(([userId, count]) => ({ userId, count }))
    .sort((a, b) => b.count - a.count || a.userId - b.userId);
  return json({ total: tasks.length, byStatus, byPriority, byAssignee });
}
