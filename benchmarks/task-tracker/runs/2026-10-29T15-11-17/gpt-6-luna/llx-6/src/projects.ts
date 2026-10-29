import { fail, ok, projectAccess, type Ctx, type Result } from "./http";
import { bad, iso } from "./model";

export function projectRoutes(c: Ctx, path: string): Result | undefined {
  const s = c.state;
  if (path === "/projects" && c.req.method === "POST") {
    const { name, key } = c.body ?? {};
    if (typeof name !== "string" || !name.trim() || name.trim().length > 100)
      return { status: 400, body: bad("name") };
    if (typeof key !== "string" || !/^[A-Z]{2,6}$/.test(key))
      return { status: 400, body: bad("key") };
    if (s.projects.some(p => p.key === key)) return fail(409, "conflict", "Key exists", "key");
    const p = { id: ++s.ids.project, name: name.trim(), key, ownerId: c.user!,
      createdAt: iso(s), members: new Map<number, string>(), next: 0 };
    p.members.set(c.user!, "owner"); s.projects.push(p);
    return ok({ id: p.id, name: p.name, key, ownerId: p.ownerId, createdAt: p.createdAt }, 201);
  }
  if (path === "/projects" && c.req.method === "GET") {
    return ok(s.projects.filter(p => p.members.has(c.user!)).map(({ id, name, key, ownerId, createdAt }) =>
      ({ id, name, key, ownerId, createdAt })));
  }
  const m = path.match(/^\/projects\/(\d+)(?:\/(members)(?:\/(\d+))?|\/(stats))?$/);
  if (!m) return;
  const p = projectAccess(c, Number(m[1]));
  if (!p) return fail(404, "not_found");
  if (m[2] === "members") {
    if (c.req.method === "GET") return ok([...p.members].sort((a, b) => a[0] - b[0])
      .map(([userId, role]) => ({ userId, role })));
    if (m[3] && c.req.method === "DELETE") {
      if (p.ownerId !== c.user) return fail(403, "forbidden");
      const id = Number(m[3]);
      if (!p.members.has(id) || id === p.ownerId) return fail(404, "not_found");
      p.members.delete(id);
      for (const t of s.tasks) if (t.projectId === p.id && t.assigneeId === id) t.assigneeId = null;
      return { status: 204 };
    }
    if (!m[3] && c.req.method === "POST") {
      if (p.ownerId !== c.user) return fail(403, "forbidden");
      const { userId, role } = c.body ?? {};
      if (!Number.isInteger(userId) || !s.users.some(u => u.id === userId))
        return { status: 400, body: bad("userId") };
      if (!["member", "viewer"].includes(role)) return { status: 400, body: bad("role") };
      if (p.members.has(userId)) return fail(409, "conflict");
      p.members.set(userId, role); return ok({ userId, role }, 201);
    }
  }
  if (m[4] === "stats" && c.req.method === "GET") {
    const ts = s.tasks.filter(t => t.projectId === p.id && !t.deleted);
    const counts = (field: string, values: string[]) =>
      Object.fromEntries(values.map(v => [v, ts.filter(t => (t as any)[field] === v).length]));
    const assignees = new Map<number, number>();
    ts.forEach(t => { if (t.assigneeId) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0) + 1); });
    return ok({ total: ts.length, byStatus: counts("status", ["open", "in_progress", "done"]),
      byPriority: counts("priority", ["high", "normal", "low"]),
      byAssignee: [...assignees].map(([userId, count]) => ({ userId, count }))
        .sort((a, b) => b.count - a.count || a.userId - b.userId) });
  }
  if (c.req.method === "GET") {
    const { id, name, key, ownerId, createdAt } = p;
    return ok({ id, name, key, ownerId, createdAt });
  }
}
