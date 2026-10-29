import { auth, fail, obj, projectAccess, valid } from "./common";
import { iso, type State } from "./model";
export async function projects(s: State, req: Request, url: URL, path: string) {
  const method = req.method;
  if (path === "/projects" && method === "GET") {
    const id = auth(s, req);
    return [200, s.projects.filter(p => s.members.get(p.id)?.has(id))];
  }
  if (path === "/projects" && method === "POST") {
    const id = auth(s, req), b = obj(await req.json().catch(() => fail(400, "validation")));
    valid(typeof b.name === "string" && b.name.trim().length > 0 && b.name.trim().length <= 100, "name");
    valid(typeof b.key === "string" && /^[A-Z]{2,6}$/.test(b.key), "key");
    if (s.projects.some(p => p.key === b.key)) fail(409, "conflict", "key");
    const p = { id: s.nextProject++, name: b.name.trim(), key: b.key, ownerId: id, createdAt: iso(s) };
    s.projects.push(p); s.members.set(p.id, new Map([[id, "owner"]]));
    return [201, p];
  }
  const m = /^\/projects\/([^/]+)(?:\/(members)(?:\/([^/]+))?|\/(stats)|\/(tasks)(?:\/(bulk))?)?$/.exec(path);
  if (!m) return null;
  const pid = /^\d+$/.test(m[1]!) ? Number(m[1]) : NaN;
  const user = auth(s, req), p = projectAccess(s, pid, user)!;
  if (!m[2] && !m[4] && !m[5]) return method === "GET" ? [200, p] : null;
  if (m[2] === "members") {
    const role = s.members.get(pid)!;
    if (method === "GET" && !m[3]) {
      const ordered = [...role].sort((a, b) => a[1] === "owner" ? -1 :
        b[1] === "owner" ? 1 : a[0] - b[0]);
      return [200, ordered.map(([userId, r]) => ({ userId, role: r }))];
    }
    if (method === "POST" && !m[3]) {
      if (p.ownerId !== user) fail(403, "forbidden");
      const b = obj(await req.json().catch(() => fail(400, "validation")));
      valid(Number.isInteger(b.userId) && s.users.some(u => u.id === b.userId), "userId");
      valid(["member", "viewer"].includes(b.role), "role");
      if (role.has(b.userId)) fail(409, "conflict");
      role.set(b.userId, b.role); return [201, { userId: b.userId, role: b.role }];
    }
    if (method === "DELETE" && m[3]) {
      if (p.ownerId !== user) fail(403, "forbidden");
      const uid = Number(m[3]);
      if (!role.has(uid) || uid === p.ownerId) fail(404, "not_found");
      role.delete(uid);
      s.tasks.filter(t => t.projectId === pid && t.assigneeId === uid).forEach(t => { t.assigneeId = null; });
      return [204, null];
    }
  }
  if (m[4] === "stats" && method === "GET") {
    const tasks = s.tasks.filter(t => t.projectId === pid);
    const tally = (field: "status" | "priority", values: string[]) =>
      Object.fromEntries(values.map(v => [v, tasks.filter(t => t[field] === v).length]));
    const counts = new Map<number, number>();
    tasks.forEach(t => { if (t.assigneeId) counts.set(t.assigneeId, (counts.get(t.assigneeId) || 0) + 1); });
    return [200, { total: tasks.length, byStatus: tally("status", ["open", "in_progress", "done"]),
      byPriority: tally("priority", ["high", "normal", "low"]),
      byAssignee: [...counts].map(([userId, count]) => ({ userId, count }))
        .sort((a, b) => b.count - a.count || a.userId - b.userId) }];
  }
  if (m[5] === "tasks" && method === "GET") return null;
  return null;
}
