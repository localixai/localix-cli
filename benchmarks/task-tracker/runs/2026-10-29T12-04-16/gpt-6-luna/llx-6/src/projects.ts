import { type Ctx, auth, body, fail, noContent, ok, valid } from "./http";
import { iso } from "./model";
export async function projects(c: Ctx, path: string, method: string): Promise<Response | null> {
  const { s } = c;
  const rm = path.match(/^\/projects\/(\d+)\/members\/(\d+)$/);
  if (rm && method === "DELETE") {
    const pid = Number(rm[1]), uid = Number(rm[2]);
    auth(c, "owner", pid);
    if (!s.members.get(pid)!.has(uid) || s.projects.find((x) => x.id === pid)!.ownerId === uid) {
      fail(404, "not_found", "Member not found");
    }
    s.members.get(pid)!.delete(uid);
    for (const t of s.tasks) if (t.projectId === pid && t.assigneeId === uid) t.assigneeId = null;
    return noContent();
  }
  if (path === "/projects" && method === "POST") {
    const user = auth(c), b = await body(c);
    valid(typeof b.name === "string" && b.name.trim().length > 0 && b.name.trim().length <= 100, "name");
    valid(typeof b.key === "string" && /^[A-Z]{2,6}$/.test(b.key), "key");
    if (s.projects.some((p) => p.key === b.key)) fail(409, "conflict", "Key exists", "key");
    const p = { id: ++s.ids.project, name: b.name.trim(), key: b.key, ownerId: user, createdAt: iso(s) };
    s.projects.push(p); s.members.set(p.id, new Map([[user, "owner"]]));
    return ok(p, 201);
  }
  if (path === "/projects" && method === "GET") {
    const user = auth(c);
    return ok(s.projects.filter((p) => s.members.get(p.id)?.has(user)));
  }
  const m = path.match(/^\/projects\/([^/]+)(?:\/(members|stats|tasks(?:\/bulk)?|tasks))?$/);
  if (!m) return null;
  const id = Number(m[1]), p = s.projects.find((x) => x.id === id);
  if (!Number.isInteger(id) || !p) { auth(c); fail(404, "not_found", "Project not found"); }
  const suffix = m[2] ?? "";
  if (!suffix && method === "GET") { auth(c, undefined, id); return ok(p); }
  if (suffix === "members") {
    if (method === "GET") {
      auth(c, undefined, id);
      return ok([...s.members.get(id)!.entries()].sort(([a], [b]) => a - b)
        .map(([userId, role]) => ({ userId, role })));
    }
    if (method === "POST") {
      auth(c, "owner", id); const b = await body(c);
      valid(["member", "viewer"].includes(b.role), "role");
      valid(s.users.some((u) => u.id === b.userId), "userId");
      const members = s.members.get(id)!;
      if (members.has(b.userId)) fail(409, "conflict", "Already a member");
      members.set(b.userId, b.role); return ok({ userId: b.userId, role: b.role }, 201);
    }
  }
  return null;
}
