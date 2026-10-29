import { State, Project, membership, iso, bad, error, notFound } from "./model";

export function projectRoute(s: State, method: string, path: string, user: number,
  data: Record<string, any> | null): Response | undefined {
  if (path === "/projects" && method === "POST") {
    const name = typeof data?.name === "string" ? data.name.trim() : "";
    const key = data?.key;
    if (!name || name.length > 100) return bad("name");
    if (typeof key !== "string" || !/^[A-Z]{2,6}$/.test(key)) return bad("key");
    if (s.projects.some(p => p.key === key)) return error("conflict", "Key is taken", 409, "key");
    const p = { id: ++s.ids.project, name, key, ownerId: user, createdAt: iso(s) };
    s.projects.push(p);
    s.members.set(p.id, new Map([[user, "owner"]]));
    return Response.json(p, { status: 201 });
  }
  if (path === "/projects" && method === "GET")
    return Response.json(s.projects.filter(p => membership(s, p, user)));
  const m = path.match(/^\/projects\/(\d+)(?:\/(members|stats)(?:\/\d+)?)?$/);
  if (!m) return;
  const p = s.projects.find(x => x.id === Number(m[1]));
  if (!p || !membership(s, p, user)) return notFound();
  if (!m[2] && method === "GET") return Response.json(p);
  if (m[2] === "members") {
    const members = s.members.get(p.id)!;
    if (method === "GET") return Response.json([...members].sort((a, b) => a[0] - b[0])
      .map(([userId, role]) => ({ userId, role })));
    if (method === "POST") {
      if (p.ownerId !== user) return error("forbidden", "Forbidden", 403);
      if (!data) return bad("body");
      const id = data?.userId;
      if (!Number.isInteger(id) || !s.users.some(u => u.id === id)) return bad("userId");
      if (!["member", "viewer"].includes(data?.role)) return bad("role");
      if (members.has(id)) return error("conflict", "Already a member", 409);
      members.set(id, data.role);
      return Response.json({ userId: id, role: data.role }, { status: 201 });
    }
  }
  const remove = path.match(/^\/projects\/\d+\/members\/(\d+)$/);
  if (remove && method === "DELETE") {
    if (p.ownerId !== user) return error("forbidden", "Forbidden", 403);
    const id = Number(remove[1]), members = s.members.get(p.id)!;
    if (!members.has(id) || id === p.ownerId) return notFound();
    members.delete(id);
    s.tasks.filter(t => t.projectId === p.id && t.assigneeId === id)
      .forEach(t => { t.assigneeId = null; });
    return new Response(null, { status: 204 });
  }
  if (m[2] === "stats" && method === "GET") {
    const tasks = s.tasks.filter(t => t.projectId === p.id);
    const count = (field: string, values: string[]) =>
      Object.fromEntries(values.map(v => [v, tasks.filter(t => (t as any)[field] === v).length]));
    const assignees = new Map<number, number>();
    tasks.forEach(t => { if (t.assigneeId) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0) + 1); });
    return Response.json({ total: tasks.length,
      byStatus: count("status", ["open", "in_progress", "done"]),
      byPriority: count("priority", ["high", "normal", "low"]),
      byAssignee: [...assignees].map(([userId, count]) => ({ userId, count }))
        .sort((a, b) => b.count - a.count || a.userId - b.userId) });
  }
}
