import type { Ctx } from './model';
import { json, project, user } from './http';
export function statsRoute(c: Ctx, req: Request, path: string) {
  const m = path.match(/^\/projects\/(\d+)\/stats$/); if (!m || req.method !== 'GET') return null;
  const u = user(c, req), p = project(c, m[1]!, u), tasks = c.s.tasks.filter(t => t.projectId === p.id);
  const byStatus = { open: 0, in_progress: 0, done: 0 }, byPriority = { high: 0, normal: 0, low: 0 };
  const assignees = new Map<number, number>();
  for (const t of tasks) { (byStatus as any)[t.status]++; (byPriority as any)[t.priority]++;
    if (t.assigneeId !== null) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) || 0) + 1); }
  return json({ total: tasks.length, byStatus, byPriority, byAssignee: [...assignees]
    .sort((a, b) => b[1] - a[1] || a[0] - b[0]).map(([userId, count]) => ({ userId, count })) });
}
