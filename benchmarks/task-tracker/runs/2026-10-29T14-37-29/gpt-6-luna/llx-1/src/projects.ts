import { body, err, json, projectFor, role, type Ctx } from "./http";
import { pair } from "./model";
export async function projects(c: Ctx, path: string) {
  if (path === "/projects" && c.req.method === "POST") {
    const b = await body(c); if (!b) return err("validation", "Invalid JSON");
    const name = typeof b.name === "string" ? b.name.trim() : "";
    if (!name || name.length > 100) return err("validation", "Invalid name", "name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) return err("validation", "Invalid key", "key");
    if (c.s.projects.some(p => p.key === b.key)) return err("conflict", "Key exists", "key", 409);
    const p = { id: c.s.projects.length + 1, name, key: b.key, ownerId: c.user!, createdAt: c.now, next: 1 };
    c.s.projects.push(p); c.s.members.set(pair(p.id, c.user!), "owner");
    const { next, ...out } = p; return json(out, 201);
  }
  if (path === "/projects" && c.req.method === "GET") {
    return json(c.s.projects.filter(p => c.s.members.has(pair(p.id, c.user!))).map(({ next, ...p }) => p));
  }
  if (path.match(/^\/projects\/\d+\/members\/\d+$/) && c.req.method === "DELETE") {
    const mm = path.match(/^\/projects\/(\d+)\/members\/(\d+)$/)!;
    const pid = Number(mm[1]), uid = Number(mm[2]);
    if (role(c, pid) !== "owner") return err("forbidden", "Forbidden", undefined, 403);
    if (!c.s.members.has(pair(pid, uid)) || c.s.members.get(pair(pid, uid)) === "owner")
      return err("not_found", "Not found", undefined, 404);
    c.s.members.delete(pair(pid, uid));
    c.s.tasks.filter(t => t.projectId === pid && t.assigneeId === uid).forEach(t => { t.assigneeId = null; });
    return new Response(null, { status: 204 });
  }
  const m = path.match(/^\/projects\/(\d+)(?:\/(members|stats|tasks|tasks\/bulk))?$/);
  if (!m) return null;
  const id = Number(m[1]), p = projectFor(c, id);
  if (!p) return err("not_found", "Not found", undefined, 404);
  const suffix = m[2];
  if (!suffix && c.req.method === "GET") { const { next, ...out } = p; return json(out); }
  if (suffix === "members") {
    if (c.req.method === "GET") return json([...c.s.members].filter(([k]) => k.startsWith(`${id}:`))
      .map(([k, role]) => ({ userId: Number(k.split(":")[1]), role }))
      .sort((a, b) => (a.role === "owner" ? -1 : 0) - (b.role === "owner" ? -1 : 0) || a.userId - b.userId));
    if (c.req.method === "POST") {
      if (role(c, id) !== "owner") return err("forbidden", "Forbidden", undefined, 403);
      const b = await body(c);
      if (!["member", "viewer"].includes(b?.role)) return err("validation", "Invalid role", "role");
      if (!c.s.users.some(u => u.id === b.userId)) return err("validation", "Unknown user", "userId");
      if (c.s.members.has(pair(id, b.userId))) return err("conflict", "Already a member", undefined, 409);
      c.s.members.set(pair(id, b.userId), b.role); return json({ userId: b.userId, role: b.role }, 201);
    }
  }
  if (suffix === "stats" && c.req.method === "GET") {
    const ts = c.s.tasks.filter(t => t.projectId === id && !t.deleted);
    const counts = (field: string, values: string[]) => Object.fromEntries(values.map(v => [v, ts.filter(t => (t as any)[field] === v).length]));
    const assignees = new Map<number, number>();
    ts.forEach(t => { if (t.assigneeId) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) || 0) + 1); });
    return json({ total: ts.length, byStatus: counts("status", ["open", "in_progress", "done"]),
      byPriority: counts("priority", ["high", "normal", "low"]),
      byAssignee: [...assignees].map(([userId, count]) => ({ userId, count }))
        .sort((a, b) => b.count - a.count || a.userId - b.userId) });
  }
  return null;
}
