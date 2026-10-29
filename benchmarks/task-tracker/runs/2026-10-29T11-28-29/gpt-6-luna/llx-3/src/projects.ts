import { State, Project, bad, conflict, error, iso, json, memberRole } from "./model";
export function projectRoutes(s: State, method: string, path: string, body: any, uid: number) {
  const removal = path.match(/^\/projects\/(\d+)\/members\/(\d+)$/);
  if (removal && method === "DELETE") {
    const project = s.projects.find((x) => x.id === Number(removal[1]));
    const target = Number(removal[2]);
    if (!project || !memberRole(s, project, uid)) return error("not_found", "Project not found", 404);
    if (uid !== project.ownerId) return error("forbidden", "Forbidden", 403);
    const members = s.members.get(project.id)!;
    if (!members.has(target) || target === project.ownerId) {
      return error("not_found", "Member not found", 404);
    }
    members.delete(target);
    s.tasks.filter((t) => t.projectId === project.id && t.assigneeId === target)
      .forEach((t) => { t.assigneeId = null; });
    return new Response(null, { status: 204 });
  }
  if (path === "/projects" && method === "POST") {
    const name = typeof body.name === "string" ? body.name.trim() : "";
    if (!name || name.length > 100) return bad("name");
    if (typeof body.key !== "string" || !/^[A-Z]{2,6}$/.test(body.key)) return bad("key");
    if (s.projects.some((p) => p.key === body.key)) return conflict("key");
    const p: Project = { id: ++s.pid, name, key: body.key, ownerId: uid, createdAt: iso(s) };
    s.projects.push(p); s.members.set(p.id, new Map([[uid, "owner"]]));
    return json(p, 201);
  }
  if (path === "/projects" && method === "GET") {
    return json(s.projects.filter((p) => memberRole(s, p, uid)).sort((a,b) => a.id-b.id));
  }
  const m = path.match(/^\/projects\/(\d+)(?:\/(members|stats))?$/);
  if (!m) return;
  const p = s.projects.find((x) => x.id === Number(m[1]));
  if (!p || !memberRole(s, p, uid)) return error("not_found", "Project not found", 404);
  if (!m[2] && method === "GET") return json(p);
  if (m[2] === "members") {
    const members = s.members.get(p.id)!;
    if (method === "GET") return json([...members].sort((a, b) => a[0] - b[0])
      .map(([userId, role]) => ({ userId, role })));
    if (method === "POST") {
      if (uid !== p.ownerId) return error("forbidden", "Forbidden", 403);
      if (!Number.isInteger(body.userId) || !s.users.some((u) => u.id === body.userId)) return bad("userId");
      if (!["member", "viewer"].includes(body.role)) return bad("role");
      if (members.has(body.userId)) return conflict();
      members.set(body.userId, body.role); return json({ userId: body.userId, role: body.role }, 201);
    }
  }
  if (m[2] === "stats" && method === "GET") {
    const ts = s.tasks.filter((t) => t.projectId === p.id);
    const counts = (key: string, values: string[]) => Object.fromEntries(
      values.map((v) => [v, ts.filter((t) => (t as any)[key] === v).length]));
    const assignees = new Map<number, number>();
    ts.forEach((t) => { if (t.assigneeId) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) || 0) + 1); });
    return json({ total: ts.length, byStatus: counts("status", ["open", "in_progress", "done"]),
      byPriority: counts("priority", ["high", "normal", "low"]),
      byAssignee: [...assignees].sort((a,b) => b[1]-a[1] || a[0]-b[0]).map(([userId,count]) => ({ userId,count })) });
  }
}
