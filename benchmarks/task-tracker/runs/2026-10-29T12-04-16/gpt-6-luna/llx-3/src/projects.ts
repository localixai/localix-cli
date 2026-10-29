import type { State } from "./model";
import { access, answer, fail, member, stamp } from "./common";

export async function projectRoutes(s: State, method: string, path: string, uid: number,
  readBody: () => Promise<Record<string, any> | null>) {
  if (path === "/projects" && method === "POST") {
    const b = await readBody();
    if (!b || typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100)
      return fail(400, "validation", "Invalid name", "name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key))
      return fail(400, "validation", "Invalid key", "key");
    if (s.projects.some(p => p.key === b.key)) return fail(409, "conflict", "Key taken", "key");
    const p = { id: s.projects.length + 1, name: b.name.trim(), key: b.key,
      ownerId: uid, createdAt: stamp(s) };
    s.projects.push(p);
    s.members.set(p.id, new Map([[uid, "owner"]]));
    return answer(p, 201);
  }
  if (path === "/projects" && method === "GET")
    return answer(s.projects.filter(p => access(s, p.id, uid)));
  const match = path.match(/^\/projects\/(\d+)(?:\/(members)(?:\/(\d+))?)?$/);
  if (!match) return null;
  const id = Number(match[1]), project = s.projects.find(p => p.id === id);
  if (method === "GET" && !match[2])
    return project && access(s, id, uid) ? answer(project) : fail(404, "not_found");
  if (method === "GET" && match[2]) {
    if (!project || !access(s, id, uid)) return fail(404, "not_found");
    const members = s.members.get(id) ?? new Map();
    return answer([...members].map(([userId, role]) => ({ userId, role }))
      .sort((a, b) => a.role === "owner" ? -1 : b.role === "owner" ? 1 : a.userId - b.userId));
  }
  if (!project) return fail(404, "not_found");
  if (method === "POST" && match[2]) {
    if (project.ownerId !== uid) return fail(403, "forbidden");
    const b = await readBody(), userId = b?.userId, role = b?.role;
    if (!["member", "viewer"].includes(role)) return fail(400, "validation", "Invalid role", "role");
    if (!Number.isInteger(userId) || !s.users.some(u => u.id === userId))
      return fail(400, "validation", "Invalid user", "userId");
    const members = s.members.get(id)!;
    if (members.has(userId)) return fail(409, "conflict");
    members.set(userId, role);
    return answer({ userId, role }, 201);
  }
  if (method === "DELETE" && match[2] && match[3]) {
    if (project.ownerId !== uid) return fail(403, "forbidden");
    const userId = Number(match[3]), members = s.members.get(id)!;
    if (!members.has(userId) || userId === uid) return fail(404, "not_found");
    members.delete(userId);
    s.tasks.filter(t => t.projectId === id && t.assigneeId === userId)
      .forEach(t => { t.assigneeId = null; });
    return answer(null, 204);
  }
  return null;
}
