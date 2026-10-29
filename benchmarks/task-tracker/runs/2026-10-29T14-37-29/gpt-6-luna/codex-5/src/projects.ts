import {
  body,
  err,
  iso,
  json,
  projectFor,
  validation,
  type Ctx,
} from "./model";
export async function projects(c: Ctx, path: string, method: string) {
  if (path === "/projects" && method === "POST") {
    const b = await body(c);
    if (!b) return validation();
    if (
      typeof b.name !== "string" ||
      !b.name.trim() ||
      b.name.trim().length > 100
    )
      return validation("name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key))
      return validation("key");
    if (c.db.projects.some((p) => p.key === b.key))
      return err("conflict", 409, "key");
    const p = {
      id: c.db.projects.length + 1,
      name: b.name.trim(),
      key: b.key,
      ownerId: c.user!.id,
      createdAt: iso(c),
      next: 1,
    };
    c.db.projects.push(p);
    c.db.members.set(p.id, new Map([[p.ownerId, "owner"]]));
    const { next, ...out } = p;
    return json(out, 201);
  }
  if (path === "/projects" && method === "GET")
    return json(
      c.db.projects
        .filter((p) => c.db.members.get(p.id)?.has(c.user!.id))
        .map(({ next, ...p }) => p),
    );
  const m = path.match(
    /^\/projects\/([^/]+)(?:\/(members|stats)(?:\/([^/]+))?)?$/,
  );
  if (!m) return null;
  const id = /^\d+$/.test(m[1]!) ? Number(m[1]) : -1;
  const p = projectFor(c, id);
  if (!p) return err("not_found", 404);
  const suffix = m[2];
  if (!suffix && method === "GET") {
    const { next, ...out } = p;
    return json(out);
  }
  if (suffix === "members") {
    if (method === "GET")
      return json(
        [...c.db.members.get(id)!.entries()]
          .sort(([a], [b]) =>
            a === p.ownerId ? -1 : b === p.ownerId ? 1 : a - b,
          )
          .map(([userId, role]) => ({
            userId,
            role,
          })),
      );
    if (method === "POST") {
      if (c.user!.id !== p.ownerId) return err("forbidden", 403);
      const b = await body(c);
      if (!b) return validation();
      if (
        !Number.isInteger(b.userId) ||
        !c.db.users.some((u) => u.id === b.userId)
      )
        return validation("userId");
      if (!["member", "viewer"].includes(b.role)) return validation("role");
      const map = c.db.members.get(id)!;
      if (map.has(b.userId)) return err("conflict", 409);
      map.set(b.userId, b.role);
      return json({ userId: b.userId, role: b.role }, 201);
    }
    const del = path.match(/\/members\/(\d+)$/);
    if (del && method === "DELETE") {
      if (c.user!.id !== p.ownerId) return err("forbidden", 403);
      const uid = Number(del[1]),
        map = c.db.members.get(id)!;
      if (!map.has(uid) || uid === p.ownerId) return err("not_found", 404);
      map.delete(uid);
      c.db.tasks
        .filter((t) => t.projectId === id && t.assigneeId === uid)
        .forEach((t) => (t.assigneeId = null));
      return new Response(null, { status: 204 });
    }
  }
  if (suffix === "stats" && method === "GET") {
    const tasks = c.db.tasks.filter((t) => t.projectId === id && !t.deleted);
    const count = (field: string, values: string[]) =>
      Object.fromEntries(
        values.map((v) => [
          v,
          tasks.filter((t) => (t as any)[field] === v).length,
        ]),
      );
    const as = new Map<number, number>();
    tasks.forEach((t) => {
      if (t.assigneeId) as.set(t.assigneeId, (as.get(t.assigneeId) || 0) + 1);
    });
    return json({
      total: tasks.length,
      byStatus: count("status", ["open", "in_progress", "done"]),
      byPriority: count("priority", ["high", "normal", "low"]),
      byAssignee: [...as]
        .map(([userId, count]) => ({ userId, count }))
        .sort((a, b) => b.count - a.count || a.userId - b.userId),
    });
  }
  return null;
}
