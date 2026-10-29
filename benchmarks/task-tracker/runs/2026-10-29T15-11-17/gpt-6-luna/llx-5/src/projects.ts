import type { Store, Project } from "./model";
import { iso, memberRole } from "./model";
import { auth, fail, ok, readBody, required } from "./http";

export async function projects(req: Request, url: URL, s: Store) {
  const path = url.pathname;
  if (path === "/projects" && req.method === "POST") {
    const u = auth(req, s), b = await readBody(req);
    const name = required(b.name, "name", (v) =>
      typeof v === "string" && v.trim().length > 0 && v.trim().length <= 100);
    const key = required(b.key, "key", (v) => typeof v === "string" && /^[A-Z]{2,6}$/.test(v));
    if (s.projects.some((p) => p.key === key)) fail(409, "conflict", "key already exists", "key");
    const p: Project = { id: ++s.pid, name: name.trim(), key, ownerId: u.id, createdAt: iso(s), next: 1 };
    s.projects.push(p); s.members.set(p.id, new Map());
    return ok(publicProject(p), 201);
  }
  if (path === "/projects" && req.method === "GET") {
    const u = auth(req, s);
    return ok(s.projects.filter((p) => memberRole(s, p, u.id)).map(publicProject));
  }
  const match = path.match(/^\/projects\/(\d+)(?:\/(members)(?:\/(\d+))?|\/(stats))?$/);
  if (!match) return null;
  const u = auth(req, s), p = s.projects.find((x) => x.id === Number(match[1]));
  if (!p || !memberRole(s, p, u.id)) fail(404, "not_found", "not found");
  if (!match[2] && !match[4] && req.method === "GET") return ok(publicProject(p));
  if (match[4] && req.method === "GET") return ok(stats(s, p));
  if (match[2] === "members" && req.method === "GET") {
    const members = [{ userId: p.ownerId, role: "owner" },
      ...[...s.members.get(p.id)!].sort(([a], [b]) => a - b).map(([userId, role]) => ({ userId, role }))];
    return ok(members);
  }
  if (match[2] === "members" && req.method === "POST") {
    if (p.ownerId !== u.id) fail(403, "forbidden", "forbidden");
    const b = await readBody(req);
    required(b.role, "role", (v) => v === "member" || v === "viewer");
    required(b.userId, "userId", (v) => Number.isInteger(v) && s.users.some((x) => x.id === v));
    if (memberRole(s, p, b.userId)) fail(409, "conflict", "already a member");
    s.members.get(p.id)!.set(b.userId, b.role);
    return ok({ userId: b.userId, role: b.role }, 201);
  }
  if (match[2] === "members" && match[3] && req.method === "DELETE") {
    if (p.ownerId !== u.id) fail(403, "forbidden", "forbidden");
    const id = Number(match[3]);
    if (!s.members.get(p.id)!.delete(id)) fail(404, "not_found", "not found");
    s.tasks.filter((t) => t.projectId === p.id && t.assigneeId === id).forEach((t) => { t.assigneeId = null; });
    return ok(null, 204);
  }
  return null;
}
export const publicProject = ({ id, name, key, ownerId, createdAt }: Project) =>
  ({ id, name, key, ownerId, createdAt });
function stats(s: Store, p: Project) {
  const ts = s.tasks.filter((t) => t.projectId === p.id && !t.deleted);
  const count = (field: string, values: string[]) => Object.fromEntries(values.map((v) =>
    [v, ts.filter((t) => (t as any)[field] === v).length]));
  const assignees = new Map<number, number>();
  ts.forEach((t) => {
    if (t.assigneeId !== null) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0) + 1);
  });
  return { total: ts.length, byStatus: count("status", ["open", "in_progress", "done"]),
    byPriority: count("priority", ["high", "normal", "low"]),
    byAssignee: [...assignees].map(([userId, count]) => ({ userId, count }))
      .sort((a, b) => b.count - a.count || a.userId - b.userId) };
}
