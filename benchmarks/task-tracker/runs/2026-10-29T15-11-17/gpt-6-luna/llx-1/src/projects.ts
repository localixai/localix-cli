import type { RequestCtx } from "./http";
import { bad, body, conflict, denied, empty, missing, ok } from "./http";
import { iso, memberRole, pair, projectOf } from "./model";

export async function projects(method: string, path: string, c: RequestCtx) {
  const { state: s, userId: uid, req } = c;
  if (path === "/projects" && method === "POST") {
    const b = await body(req); if (!b) return bad("body");
    if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) return bad("name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) return bad("key");
    if (s.projects.some(p => p.key === b.key)) return conflict("key");
    const p = { id: ++s.ids.project, name: b.name.trim(), key: b.key, ownerId: uid, createdAt: iso(c) };
    s.projects.push(p); return ok(p, 201);
  }
  if (path === "/projects" && method === "GET")
    return ok(s.projects.filter(p => memberRole(s, p.id, uid)).sort((a,b) => a.id-b.id));
  const m = path.match(/^\/projects\/([^/]+)(?:\/(.*))?$/);
  if (!m) return undefined;
  const id = /^\d+$/.test(m[1]!) ? Number(m[1]) : -1, p = projectOf(s, id);
  const sub = m[2] ?? "";
  if (sub === "" && method === "GET") return p && memberRole(s,id,uid) ? ok(p) : missing();
  if (!p || !memberRole(s,id,uid)) return missing();
  if (sub === "members" && method === "GET") {
    const arr = [{ userId: p.ownerId, role: "owner" }, ...Array.from(s.members)
      .filter(([k]) => k.startsWith(`${id}:`)).map(([k,role]) => ({ userId: Number(k.split(":")[1]), role }))
      .sort((a,b) => a.userId-b.userId)];
    return ok(arr);
  }
  if (sub === "members" && method === "POST") {
    if (p.ownerId !== uid) return denied();
    const b = await body(req); if (!b) return bad("body");
    if (b.role !== "member" && b.role !== "viewer") return bad("role");
    if (!s.users.some(u => u.id === b.userId)) return bad("userId");
    if (memberRole(s,id,b.userId)) return conflict();
    s.members.set(pair(id,b.userId),b.role); return ok({ userId: b.userId, role: b.role },201);
  }
  const del = sub.match(/^members\/(\d+)$/);
  if (del && method === "DELETE") {
    if (p.ownerId !== uid) return denied();
    const userId = Number(del[1]), key = pair(id,userId);
    if (!s.members.has(key)) return missing();
    s.members.delete(key);
    for (const t of s.tasks) if (t.projectId === id && t.assigneeId === userId) t.assigneeId = null;
    return empty();
  }
  return undefined;
}
