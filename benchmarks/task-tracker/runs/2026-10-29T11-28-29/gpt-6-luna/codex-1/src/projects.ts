import {
  canWrite,
  fail,
  json,
  projectAccess,
  role,
  stamp,
  type State,
} from "./model";
export function projectRoute(
  s: State,
  method: string,
  path: string,
  uid: number,
  body: any,
) {
  if (path === "/projects" && method === "POST") {
    const name = typeof body.name === "string" ? body.name.trim() : "";
    if (!name || name.length > 100)
      return fail(400, "validation", "Invalid name", "name");
    if (typeof body.key !== "string" || !/^[A-Z]{2,6}$/.test(body.key))
      return fail(400, "validation", "Invalid key", "key");
    if (s.projects.some((p) => p.key === body.key))
      return fail(409, "conflict", "Key already exists", "key");
    const p = {
      id: ++s.ids.project,
      name,
      key: body.key,
      ownerId: uid,
      createdAt: stamp(s),
    };
    s.projects.push(p);
    s.members.set(p.id, new Map());
    return json(p, 201);
  }
  if (path === "/projects" && method === "GET")
    return json(s.projects.filter((p) => role(s, p, uid)));
  const m = path.match(/^\/projects\/([^/]+)(?:\/(members)(?:\/([^/]+))?)?$/);
  if (!m) return undefined;
  const id = Number(m[1]);
  const p = Number.isInteger(id) ? projectAccess(s, uid, id) : undefined;
  if (!p) return fail(404, "not_found", "Project not found");
  if (!m[2]) return method === "GET" ? json(p) : undefined;
  if (m[2] === "members" && !m[3] && method === "GET") {
    const items = [
      { userId: p.ownerId, role: "owner" },
      ...[...(s.members.get(id) ?? [])]
        .sort((a, b) => a[0] - b[0])
        .map(([userId, r]) => ({ userId, role: r })),
    ];
    return json(items);
  }
  if (m[2] === "members" && method === "POST") {
    if (p.ownerId !== uid) return fail(403, "forbidden", "Owner required");
    if (
      !Number.isInteger(body.userId) ||
      !s.users.some((u) => u.id === body.userId)
    )
      return fail(400, "validation", "Invalid userId", "userId");
    if (body.role !== "member" && body.role !== "viewer")
      return fail(400, "validation", "Invalid role", "role");
    const members = s.members.get(id)!;
    if (body.userId === p.ownerId || members.has(body.userId))
      return fail(409, "conflict", "Already a member");
    members.set(body.userId, body.role);
    return json({ userId: body.userId, role: body.role }, 201);
  }
  if (m[2] === "members" && m[3] && method === "DELETE") {
    if (p.ownerId !== uid) return fail(403, "forbidden", "Owner required");
    const target = Number(m[3]);
    const members = s.members.get(id)!;
    if (!members.delete(target))
      return fail(404, "not_found", "Member not found");
    for (const t of s.tasks)
      if (t.projectId === id && t.assigneeId === target) t.assigneeId = null;
    return new Response(null, { status: 204 });
  }
  return undefined;
}
