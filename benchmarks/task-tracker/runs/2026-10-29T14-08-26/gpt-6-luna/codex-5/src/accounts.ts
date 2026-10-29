import type { Ctx } from "./context";
import { iso, projectDto, userDto } from "./context";
import { bad, fail, json, validText } from "./http";
export function account(
  c: Ctx,
  path: string,
  method: string,
): Response | undefined {
  const { s, body, user } = c;
  if (path === "/users" && method === "POST") {
    const email =
      typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
    if (!/^[^@]+@[^@]+\.[^@]+$/.test(email)) bad("email");
    if (typeof body.password !== "string" || body.password.length < 8)
      bad("password");
    const name = validText(body.name, "name", 100);
    if (s.users.some((u) => u.email === email))
      fail(409, "conflict", "Email already exists", "email");
    const u = { id: s.users.length + 1, email, password: body.password, name };
    s.users.push(u);
    return json(userDto(u), 201);
  }
  if (path === "/sessions" && method === "POST") {
    const email = String(body.email ?? "").toLowerCase();
    const u = s.users.find(
      (x) => x.email === email && x.password === body.password,
    );
    if (!u) return fail(401, "unauthorized", "Unauthorized");
    const token = crypto.randomUUID();
    s.sessions.set(token, u.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    s.sessions.delete(c.req.headers.get("Authorization")?.slice(7) ?? "");
    return new Response(null, { status: 204 });
  }
  if (path === "/me" && method === "GET") return json(userDto(user));
  if (path === "/projects" && method === "GET")
    return json(
      s.projects.filter((p) => p.members.has(user.id)).map(projectDto),
    );
  if (path === "/projects" && method === "POST") {
    const name = validText(body.name, "name", 100);
    if (typeof body.key !== "string" || !/^[A-Z]{2,6}$/.test(body.key))
      bad("key");
    if (s.projects.some((p) => p.key === body.key))
      fail(409, "conflict", "Key already exists", "key");
    const p = {
      id: s.projects.length + 1,
      name,
      key: body.key,
      ownerId: user.id,
      createdAt: iso(c),
      members: new Map([[user.id, "owner"]]),
      next: 1,
    };
    s.projects.push(p);
    return json(projectDto(p), 201);
  }
  const pMatch = path.match(
    /^\/projects\/([^/]+)(?:\/(members)(?:\/(\d+))?)?$/,
  );
  if (pMatch) {
    const p = s.projects.find((x) => x.id === Number(pMatch[1]));
    if (!p || !p.members.has(user.id))
      return fail(404, "not_found", "Project not found");
    if (!pMatch[2] && method === "GET") return json(projectDto(p));
    if (pMatch[2] && method === "GET")
      return json(
        [...p.members]
          .sort((a, b) => a[0] - b[0])
          .map(([userId, role]) => ({ userId, role })),
      );
    if (pMatch[2] && method === "POST") {
      if (p.ownerId !== user.id) fail(403, "forbidden", "Forbidden");
      if (
        !Number.isInteger(body.userId) ||
        !s.users.some((u) => u.id === body.userId)
      )
        bad("userId");
      if (!["member", "viewer"].includes(body.role)) bad("role");
      if (p.members.has(body.userId)) fail(409, "conflict", "Already a member");
      p.members.set(body.userId, body.role);
      return json({ userId: body.userId, role: body.role }, 201);
    }
    if (pMatch[2] && pMatch[3] && method === "DELETE") {
      if (p.ownerId !== user.id) fail(403, "forbidden", "Forbidden");
      const uid = Number(pMatch[3]);
      if (!p.members.delete(uid)) fail(404, "not_found", "Member not found");
      for (const t of s.tasks)
        if (t.projectId === p.id && t.assigneeId === uid) t.assigneeId = null;
      return new Response(null, { status: 204 });
    }
  }
  return undefined;
}
