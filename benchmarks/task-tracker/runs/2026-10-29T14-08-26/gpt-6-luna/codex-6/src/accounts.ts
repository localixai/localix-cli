import { State, pubUser, stamp } from "./state";
import { auth, bodyObj, fail, project, answer } from "./http";
export function account(
  s: State,
  r: Request,
  m: string,
  path: string,
  now: () => number,
): Response | undefined {
  let b: any;
  const seg = path.split("/").filter(Boolean);
  if (path === "/users" && m === "POST") {
    b = bodyObj(awaitless(r));
    const email =
      typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email))
      fail(400, "validation", "Invalid email", "email");
    if (typeof b.password !== "string" || b.password.length < 8)
      fail(400, "validation", "Invalid password", "password");
    if (
      typeof b.name !== "string" ||
      !b.name.trim() ||
      b.name.trim().length > 100
    )
      fail(400, "validation", "Invalid name", "name");
    if (s.users.some((u) => u.email === email))
      fail(409, "conflict", "Email already used", "email");
    const u = {
      id: s.nextUser++,
      email,
      password: b.password,
      name: b.name.trim(),
    };
    s.users.push(u);
    return answer(pubUser(u), 201);
  }
  if (path === "/sessions" && m === "POST") {
    b = bodyObj(awaitless(r));
    const u = s.users.find(
      (x) =>
        x.email === String(b.email || "").toLowerCase() &&
        x.password === b.password,
    );
    if (!u) fail(401, "unauthorized", "Unauthorized");
    const token = `session-${u!.id}-${s.sessions.size}-${now()}-${Math.random()}`;
    s.sessions.set(token, u!.id);
    return answer({ token }, 201);
  }
  if (path === "/sessions" && m === "DELETE") {
    const u = auth(s, r);
    const t = r.headers.get("Authorization")!.slice(7);
    s.sessions.delete(t);
    void u;
    return answer(null, 204);
  }
  if (path === "/me" && m === "GET") return answer(pubUser(auth(s, r)));
  if (path === "/projects" && m === "POST") {
    const u = auth(s, r);
    b = bodyObj(awaitless(r));
    if (
      typeof b.name !== "string" ||
      !b.name.trim() ||
      b.name.trim().length > 100
    )
      fail(400, "validation", "Invalid name", "name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key))
      fail(400, "validation", "Invalid key", "key");
    if (s.projects.some((p) => p.key === b.key))
      fail(409, "conflict", "Key already used", "key");
    const p = {
      id: s.nextProject++,
      name: b.name.trim(),
      key: b.key,
      ownerId: u.id,
      createdAt: stamp(now),
      seq: 0,
    };
    s.projects.push(p);
    s.members.set(p.id, new Map([[u.id, "owner"]]));
    const { seq, ...out } = p;
    return answer(out, 201);
  }
  if (path === "/projects" && m === "GET") {
    const u = auth(s, r);
    return answer(
      s.projects
        .filter((p) => s.members.get(p.id)?.has(u.id))
        .map(({ seq, ...p }) => p),
    );
  }
  if (seg[0] === "projects" && seg.length === 2 && m === "GET") {
    const u = auth(s, r),
      p = project(s, seg[1]!, u.id);
    const { seq, ...out } = p;
    return answer(out);
  }
  if (seg[0] === "projects" && seg[2] === "members")
    return members(s, r, m, seg, now);
  return undefined;
}
function awaitless(r: Request) {
  return (r as any).__body || {};
}
function members(
  s: State,
  r: Request,
  m: string,
  a: string[],
  now: () => number,
) {
  const u = auth(s, r),
    p = project(s, a[1]!, u.id),
    map = s.members.get(p.id)!;
  if (a.length === 3 && m === "GET")
    return answer(
      [...map]
        .sort((x, y) =>
          x[0] === p.ownerId ? -1 : y[0] === p.ownerId ? 1 : x[0] - y[0],
        )
        .map(([userId, role]) => ({ userId, role })),
    );
  if (a.length === 3 && m === "POST") {
    if (u.id !== p.ownerId) fail(403, "forbidden", "Forbidden");
    const b = (r as any).__body || {};
    if (!["member", "viewer"].includes(b.role))
      fail(400, "validation", "Invalid role", "role");
    if (!s.users.some((x) => x.id === b.userId))
      fail(400, "validation", "Unknown user", "userId");
    if (map.has(b.userId)) fail(409, "conflict", "Already a member");
    map.set(b.userId, b.role);
    return answer({ userId: b.userId, role: b.role }, 201);
  }
  if (a.length === 4 && m === "DELETE") {
    if (u.id !== p.ownerId) fail(403, "forbidden", "Forbidden");
    const id = Number(a[3]);
    if (!map.has(id) || id === p.ownerId) fail(404, "not_found", "Not found");
    map.delete(id);
    for (const t of s.tasks)
      if (t.projectId === p.id && t.assigneeId === id) t.assigneeId = null;
    return answer(null, 204);
  }
  return undefined;
}
