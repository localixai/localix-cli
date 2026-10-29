import type { State, Project } from "./model";
import { auth, fail, projectOf, response, time, validation } from "./http";
import { body } from "./users";

export async function projects(
  method: string, path: string[], req: Request, s: State, now: () => number,
): Promise<Response> {
  const uid = auth(req, s);
  if (path.length === 1 && path[0] === "projects") {
    if (method === "POST") {
      const b = await body(req);
      if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) validation("name");
      if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) validation("key");
      if (s.projects.some((p) => p.key === b.key)) fail(409, "conflict", "Project key exists", "key");
      const p: Project = { id: ++s.ids.project, name: b.name.trim(), key: b.key,
        ownerId: uid, createdAt: time(now), next: 1 };
      s.projects.push(p);
      s.members.set(p.id, [{ userId: uid, role: "owner" }]);
      return response(publicProject(p), 201);
    }
    if (method === "GET") return response(s.projects.filter((p) =>
      s.members.get(p.id)?.some((m) => m.userId === uid)).map(publicProject));
    return response(null, 405, { Allow: "GET, POST" });
  }
  const id = Number(path[1]);
  const p = path[1] && /^\d+$/.test(path[1]) ? projectOf(s, id, uid) : undefined;
  if (!p) fail(404, "not_found", "Project not found");
  if (path.length === 2 && method === "GET") return response(publicProject(p));
  if (path[2] === "stats" && method === "GET") {
    const tasks = s.tasks.filter((t) => t.projectId === id);
    const byStatus = Object.fromEntries(["open", "in_progress", "done"].map((x) =>
      [x, tasks.filter((t) => t.status === x).length]));
    const byPriority = Object.fromEntries(["high", "normal", "low"].map((x) =>
      [x, tasks.filter((t) => t.priority === x).length]));
    const counts = new Map<number, number>();
    tasks.forEach((t) => { if (t.assigneeId !== null) counts.set(t.assigneeId, (counts.get(t.assigneeId) || 0) + 1); });
    const byAssignee = [...counts].map(([userId, count]) => ({ userId, count }))
      .sort((a, b) => b.count - a.count || a.userId - b.userId);
    return response({ total: tasks.length, byStatus, byPriority, byAssignee });
  }
  if (path[2] === "members") {
    if (method === "GET" && path.length === 3) return response(s.members.get(id));
    if (method === "POST" && path.length === 3) {
      if (p.ownerId !== uid) fail(403, "forbidden", "Only the owner may add members");
      const b = await body(req);
      if (b.role !== "member" && b.role !== "viewer") validation("role");
      if (!Number.isInteger(b.userId) || !s.users.some((u) => u.id === b.userId)) validation("userId");
      const list = s.members.get(id)!;
      if (list.some((m) => m.userId === b.userId)) fail(409, "conflict", "Already a member");
      list.push({ userId: b.userId, role: b.role });
      list.sort((a, b) => a.userId === p.ownerId ? -1 : b.userId === p.ownerId ? 1 : a.userId - b.userId);
      return response({ userId: b.userId, role: b.role }, 201);
    }
    if (method === "DELETE" && path.length === 4) {
      if (p.ownerId !== uid) fail(403, "forbidden", "Only the owner may remove members");
      const who = Number(path[3]);
      const list = s.members.get(id)!;
      const ix = list.findIndex((m) => m.userId === who && m.role !== "owner");
      if (ix < 0) fail(404, "not_found", "Member not found");
      list.splice(ix, 1);
      for (const t of s.tasks.filter((x) => x.projectId === id && x.assigneeId === who)) t.assigneeId = null;
      return response(null, 204);
    }
    return response(null, 405, { Allow: "GET, POST, DELETE" });
  }
  return response({ error: { code: "not_found", message: "Not found" } }, 404);
}
function publicProject(p: Project) {
  const { next, ...out } = p;
  void next;
  return out;
}
