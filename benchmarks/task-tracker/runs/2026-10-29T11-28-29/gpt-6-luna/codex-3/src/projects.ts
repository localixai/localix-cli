import { createTasks, listTasks } from "./task-create";

import type { State, Task } from "./model";
import { auth, bad, bodyOf, conflict, forbidden, iso, json, member, missing, projectFor, visibleRole } from "./http";
export async function projectsRoute(state: State, request: Request, path: string, method: string, url: URL) {
  if (path === "/projects" && (method === "GET" || method === "POST")) {
    const uid = auth(state, request);
    if (method === "GET") return json(state.projects.filter((p) => visibleRole(state, p.id, uid)));
    const b = await bodyOf(request);
    if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) bad("name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) bad("key");
    if (state.projects.some((p) => p.key === b.key)) conflict("conflict", "key");
    const project = { id: state.nextProject++, name: b.name.trim(), key: b.key, ownerId: uid, createdAt: iso(state) };
    state.projects.push(project);
    state.memberships.set(project.id, new Map([[uid, "owner"]]));
    return json(project, 201);
  }
  const match = path.match(/^\/projects\/([^/]+)(?:\/(members|stats|tasks))?(?:\/([^/]+))?$/);
  if (!match) return null;
  const id = Number(match[1]);
  if (!Number.isInteger(id) || id < 1) return null;
  const uid = auth(state, request);
  const project = projectFor(state, id, uid);
  const section = match[2];
  const tail = match[3];
  if (!section && method === "GET") return json(project);
  if (section === "members") {
    const memberships = state.memberships.get(id)!;
    if (!tail && method === "GET") return json([...memberships].sort(([a], [b]) => a - b)
      .map(([userId, role]) => ({ userId, role })));
    if (!tail && method === "POST") {
      if (project.ownerId !== uid) forbidden();
      const b = await bodyOf(request);
      if (!Number.isInteger(b.userId) || !state.users.some((u) => u.id === b.userId)) bad("userId");
      if (b.role !== "member" && b.role !== "viewer") bad("role");
      if (memberships.has(b.userId)) conflict();
      memberships.set(b.userId, b.role);
      return json({ userId: b.userId, role: b.role }, 201);
    }
    if (tail && method === "DELETE") {
      if (project.ownerId !== uid) forbidden();
      const memberId = Number(tail);
      if (!memberships.has(memberId) || memberId === uid) return missing();
      memberships.delete(memberId);
      for (const task of state.tasks) if (task.projectId === id && task.assigneeId === memberId) task.assigneeId = null;
      return new Response(null, { status: 204 });
    }
  }
  if (section === "stats" && method === "GET") {
    const tasks = state.tasks.filter((t) => t.projectId === id);
    const byStatus = Object.fromEntries(["open", "in_progress", "done"].map((s) =>
      [s, tasks.filter((t) => t.status === s).length]));
    const byPriority = Object.fromEntries(["high", "normal", "low"].map((s) =>
      [s, tasks.filter((t) => t.priority === s).length]));
    const counts = new Map<number, number>();
    for (const t of tasks) if (t.assigneeId !== null) counts.set(t.assigneeId, (counts.get(t.assigneeId) ?? 0) + 1);
    const byAssignee = [...counts].map(([userId, count]) => ({ userId, count }))
      .sort((a, b) => b.count - a.count || a.userId - b.userId);
    return json({ total: tasks.length, byStatus, byPriority, byAssignee });
  }
  if (section === "tasks" && method === "GET") return listTasks(state, request, url, id);
  if (section === "tasks" && (method === "POST") && !tail) return createTasks(state, request, id, false);
  if (section === "tasks" && tail === "bulk" && method === "POST") return createTasks(state, request, id, true);
  return null;
}
