import {
  Context,
  fail,
  json,
  nextId,
  iso,
  memberRole,
  visibleProject,
} from "./model";
export function projectRoutes(
  c: Context,
  path: string,
  method: string,
  body: any,
) {
  const s = c.state,
    parts = path.split("/");
  if (path === "/projects" && method === "POST") {
    const name = typeof body.name === "string" ? body.name.trim() : "",
      key = body.key;
    if (!name || name.length > 100)
      return fail("validation", "invalid name", 400, "name");
    if (typeof key !== "string" || !/^[A-Z]{2,6}$/.test(key))
      return fail("validation", "invalid key", 400, "key");
    if (s.projects.some((p) => p.key === key))
      return fail("conflict", "key already exists", 409, "key");
    const p = {
      id: nextId(s, "project"),
      name,
      key,
      ownerId: c.userId!,
      createdAt: iso(c),
    };
    s.projects.push(p);
    s.members.set(p.id, new Map([[p.ownerId, "owner"]]));
    return json(p, 201);
  }
  if (path === "/projects" && method === "GET")
    return json(
      s.projects.filter((p) => memberRole(s, p.id, c.userId!) !== undefined),
    );
  if (parts.length < 3 || !/^\d+$/.test(parts[2]!)) return;
  const id = Number(parts[2]),
    p = visibleProject(c, id);
  if (!p) return fail("not_found", "not found", 404);
  if (parts.length === 3 && method === "GET") return json(p);
  if (parts[3] === "members") {
    if (parts.length === 4 && method === "GET")
      return json(
        [...s.members.get(id)!.entries()]
          .sort(
            (a, b) =>
              (({ owner: 0, member: 1, viewer: 2 })[a[1]] ?? 3) -
                ({ owner: 0, member: 1, viewer: 2 }[b[1]] ?? 3) || a[0] - b[0],
          )
          .map(([userId, role]) => ({
            userId,
            role,
          })),
      );
    if (parts.length === 4 && method === "POST") {
      if (p.ownerId !== c.userId)
        return fail("forbidden", "owner required", 403);
      if (
        !Number.isInteger(body.userId) ||
        !s.users.some((u) => u.id === body.userId)
      )
        return fail("validation", "unknown user", 400, "userId");
      if (!["member", "viewer"].includes(body.role))
        return fail("validation", "invalid role", 400, "role");
      const map = s.members.get(id)!;
      if (map.has(body.userId))
        return fail("conflict", "already a member", 409);
      map.set(body.userId, body.role);
      return json({ userId: body.userId, role: body.role }, 201);
    }
    if (parts.length === 5 && method === "DELETE") {
      if (p.ownerId !== c.userId)
        return fail("forbidden", "owner required", 403);
      const uid = Number(parts[4]),
        map = s.members.get(id)!;
      if (!map.has(uid) || uid === p.ownerId)
        return fail("not_found", "not found", 404);
      map.delete(uid);
      for (const t of s.tasks.filter(
        (x) => x.projectId === id && x.assigneeId === uid,
      ))
        t.assigneeId = null;
      return { status: 204 };
    }
  }
}
