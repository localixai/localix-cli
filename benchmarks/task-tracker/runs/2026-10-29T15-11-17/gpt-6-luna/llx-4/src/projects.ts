import type { Store, User } from "./model";
import { stamp } from "./model";
import { body, bad, error, json, project, member, role } from "./http";
import { taskRoutes } from "./taskRoutes";

export async function projects(req: Request, s: Store, path: string[], u: User): Promise<Response> {
  if (path.length === 1) {
    if (req.method === "GET") return json(s.projects.filter(p => member(p, u)).map(publicProject));
    const b = await body(req);
    if (!b) return bad("body");
    const name = typeof b.name === "string" ? b.name.trim() : "";
    if (!name || name.length > 100) return bad("name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) return bad("key");
    if (s.projects.some(p => p.key === b.key)) return error("conflict", "key taken", 409, "key");
    const p = { id: s.projects.length + 1, name, key: b.key, ownerId: u.id,
      createdAt: stamp(s), members: new Map<number, string>(), next: 1 };
    s.projects.push(p); return json(publicProject(p), 201);
  }
  const p = project(s, path[1] || "");
  if (!p || !member(p, u)) return error("not_found", "not found", 404);
  if (path.length === 2 && req.method === "GET") return json(publicProject(p));
  if (path[2] === "tasks" || path[2] === "stats") return taskRoutes(req, s, path, u, p);
  if (path[2] === "members") {
    if (path.length === 3 && req.method === "GET") {
      const rest = [...p.members].sort((a, b) => a[0] - b[0])
        .map(([userId, r]) => ({ userId, role: r }));
      return json([{ userId: p.ownerId, role: "owner" }, ...rest]);
    }
    if (path.length === 3 && req.method === "POST") {
      if (role(p, u) !== "owner") return error("forbidden", "forbidden", 403);
      const b = await body(req); if (!b) return bad("body");
      if (b.role !== "member" && b.role !== "viewer") return bad("role");
      if (!Number.isInteger(b.userId) || !s.users.some(x => x.id === b.userId)) return bad("userId");
      if (p.ownerId === b.userId || p.members.has(b.userId)) return error("conflict", "already a member", 409);
      p.members.set(b.userId, b.role);
      return json({ userId: b.userId, role: b.role }, 201);
    }
    if (path.length === 4 && req.method === "DELETE") {
      if (role(p, u) !== "owner") return error("forbidden", "forbidden", 403);
      const id = Number(path[3]); if (!p.members.has(id)) return error("not_found", "not found", 404);
      p.members.delete(id);
      for (const t of s.tasks) if (t.projectId === p.id && t.assigneeId === id) t.assigneeId = null;
      return new Response(null, { status: 204 });
    }
  }
  return error("not_found", "not found", 404);
}
export function publicProject(p: any) {
  return { id: p.id, name: p.name, key: p.key, ownerId: p.ownerId, createdAt: p.createdAt };
}
