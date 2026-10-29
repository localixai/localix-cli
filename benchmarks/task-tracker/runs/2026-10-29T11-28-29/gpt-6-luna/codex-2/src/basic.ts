import { iso, membership, type Project } from "./model";
import {
  bad,
  conflict,
  denied,
  empty,
  err,
  member,
  missing,
  ok,
  type Ctx,
} from "./http";

export function basic(c: Ctx, path: string, method: string) {
  const { s, b, user } = { s: c.s, b: c.body, user: c.user };
  if (path === "/users" && method === "POST") {
    if (typeof b?.email !== "string" || !/^\S+@\S+\.\S+$/.test(b.email))
      return bad("email");
    if (typeof b.password !== "string" || b.password.length < 8)
      return bad("password");
    if (
      typeof b.name !== "string" ||
      !b.name.trim() ||
      b.name.trim().length > 100
    )
      return bad("name");
    const email = b.email.trim().toLowerCase();
    if (s.users.some((x) => x.email === email)) return conflict("email");
    const u = {
      id: ++s.ids.user,
      email,
      password: b.password,
      name: b.name.trim(),
    };
    s.users.push(u);
    return ok({ id: u.id, email: u.email, name: u.name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const u = s.users.find(
      (x) =>
        x.email === String(b?.email || "").toLowerCase() &&
        x.password === b?.password,
    );
    if (!u) return err(401, "unauthorized", "Unauthorized");
    const token = `token-${u.id}-${s.now()}-${Math.random()}`;
    s.sessions.set(token, u.id);
    return ok({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const t = c.req.headers.get("Authorization")?.slice(7);
    if (t) s.sessions.delete(t);
    return empty();
  }
  if (path === "/me" && method === "GET")
    return ok(publicUser(s.users.find((x) => x.id === user)!));
  if (path === "/projects" && method === "POST") {
    if (
      typeof b?.name !== "string" ||
      !b.name.trim() ||
      b.name.trim().length > 100
    )
      return bad("name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key))
      return bad("key");
    if (s.projects.some((p) => p.key === b.key)) return conflict("key");
    const p: Project = {
      id: ++s.ids.project,
      name: b.name.trim(),
      key: b.key,
      ownerId: user!,
      createdAt: iso(s),
      next: 1,
    };
    s.projects.push(p);
    s.members.set(p.id, new Map());
    return ok(publicProject(p), 201);
  }
  if (path === "/projects" && method === "GET")
    return ok(
      s.projects.filter((p) => membership(s, p, user!)).map(publicProject),
    );
  const pm = path.match(/^\/projects\/(\d+)(?:\/(members|tasks|stats))?$/);
  if (pm) {
    const id = Number(pm[1]),
      sub = pm[2],
      p = s.projects.find((x) => x.id === id);
    if (!p || !member(c, id)) return missing();
    if (!sub && method === "GET") return ok(publicProject(p));
    if (sub === "members" && method === "GET")
      return ok([
        { userId: p.ownerId, role: "owner" },
        ...[...(s.members.get(id) || [])]
          .sort((a, b) => a[0] - b[0])
          .map(([userId, role]) => ({ userId, role })),
      ]);
    if (sub === "members" && method === "POST") {
      if (p.ownerId !== user) return denied();
      if (
        !Number.isInteger(b?.userId) ||
        !s.users.some((u) => u.id === b.userId)
      )
        return bad("userId");
      if (!["member", "viewer"].includes(b.role)) return bad("role");
      const m = s.members.get(id)!;
      if (b.userId === p.ownerId || m.has(b.userId)) return conflict();
      m.set(b.userId, b.role);
      return ok({ userId: b.userId, role: b.role }, 201);
    }
    const rm = path.match(/\/members\/(\d+)$/);
    if (rm && method === "DELETE") {
      if (p.ownerId !== user) return denied();
      const uid = Number(rm[1]),
        m = s.members.get(id)!;
      if (!m.has(uid)) return missing();
      m.delete(uid);
      for (const t of s.tasks)
        if (t.projectId === id && t.assigneeId === uid) t.assigneeId = null;
      return empty();
    }
  }
  return undefined;
}
function publicUser(u: any) {
  return { id: u.id, email: u.email, name: u.name };
}
export function publicProject(p: Project) {
  return {
    id: p.id,
    name: p.name,
    key: p.key,
    ownerId: p.ownerId,
    createdAt: p.createdAt,
  };
}
