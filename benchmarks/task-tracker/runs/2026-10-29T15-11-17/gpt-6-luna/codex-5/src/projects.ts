import { auth, body, date, fail, json, project, role, validate, type Ctx } from "./util";
export async function projects(c: Ctx, method: string, path: string): Promise<Response | undefined> {
  const s = c.state;
  const match = /^\/projects\/(\d+)(?:\/(members|tasks|stats)(?:\/(\d+))?)?$/.exec(path);
  if (path === "/projects" && method === "POST") {
    const user = auth(c).user!; const b = await body(c.request);
    validate(typeof b.name === "string" && b.name.trim().length > 0 && b.name.trim().length <= 100, "name");
    validate(typeof b.key === "string" && /^[A-Z]{2,6}$/.test(b.key), "key");
    if (s.projects.some((p) => p.key === b.key)) fail(409, "conflict", "Key already exists", "key");
    const p = { id: s.counters.project = (s.counters.project || 0) + 1, name: b.name.trim(), key: b.key,
      ownerId: user.id, createdAt: date(c) };
    s.projects.push(p); s.members.set(p.id, new Map([[user.id, "owner"]])); return json(p, 201);
  }
  if (path === "/projects" && method === "GET") {
    const u = auth(c).user!; return json(s.projects.filter((p) => s.members.get(p.id)?.has(u.id)));
  }
  if (!match) return;
  const id = Number(match[1]);
  if (!match[2] && method === "GET") return json(project(c, id));
  const section = match[2];
  if (section === "members") {
    project(c, id); const members = s.members.get(id)!;
    if (method === "GET") return json([...members].sort(([a], [b]) => a - b)
      .map(([userId, role]) => ({ userId, role })));
    if (method === "POST") {
      if (role(c, id) !== "owner") fail(403, "forbidden", "Owner required");
      const b = await body(c.request); validate(["member", "viewer"].includes(b.role), "role");
      validate(Number.isInteger(b.userId) && s.users.some((u) => u.id === b.userId), "userId");
      if (members.has(b.userId)) fail(409, "conflict", "Already a member");
      members.set(b.userId, b.role); return json({ userId: b.userId, role: b.role }, 201);
    }
    if (method === "DELETE" && match[3]) {
      if (role(c, id) !== "owner") fail(403, "forbidden", "Owner required");
      const uid = Number(match[3]!);
      const ownerId = project(c, id).ownerId;
      if (!members.has(uid) || uid === ownerId) fail(404, "not_found", "Member not found");
      members.delete(uid);
      for (const t of s.tasks.filter((x) => x.projectId === id && x.assigneeId === uid)) {
        t.assigneeId = null;
      }
      return new Response(null, { status: 204 });
    }
  }
  if (section === "stats" && method === "GET") {
    project(c, id); const ts = s.tasks.filter((t) => t.projectId === id);
    const bucket = (field: string, values: string[]) => Object.fromEntries(values.map((v) =>
      [v, ts.filter((t) => (t as any)[field] === v).length]));
    const count = new Map<number, number>();
    for (const t of ts) {
      if (t.assigneeId) count.set(t.assigneeId, (count.get(t.assigneeId) || 0) + 1);
    }
    return json({ total: ts.length, byStatus: bucket("status", ["open", "in_progress", "done"]),
      byPriority: bucket("priority", ["high", "normal", "low"]),
      byAssignee: [...count].map(([userId, count]) => ({ userId, count })).sort((a, b) =>
        b.count - a.count || a.userId - b.userId) });
  }
}
