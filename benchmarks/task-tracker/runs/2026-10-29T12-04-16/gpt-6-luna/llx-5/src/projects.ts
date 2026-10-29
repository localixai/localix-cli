import { auth, json } from "./response";
import { iso, member, validate, type Ctx, type Project } from "./model";
export function projectRoute(c: Ctx, path: string) {
  const { s, body } = c;
  if (path === "/projects" && c.req.method === "POST") {
    auth(c);
    const name = typeof body.name === "string" ? body.name.trim() : "";
    validate(name.length > 0 && name.length <= 100, "name");
    validate(typeof body.key === "string" && /^[A-Z]{2,6}$/.test(body.key), "key");
    if (s.projects.some(p => p.key === body.key))
      throw Object.assign(Error("Key already taken"), {
        status: 409, code: "conflict", field: "key",
      });
    const p: Project = {
      id: (s.ids.project = s.ids.project! + 1), name, key: body.key,
      ownerId: c.user!.id, createdAt: iso(s),
    };
    s.projects.push(p); return json(p, 201);
  }
  if (path === "/projects" && c.req.method === "GET") {
    auth(c); return json(s.projects.filter(p => member(s, p, c.user!)));
  }
  const m = path.match(/^\/projects\/(\d+)(?:\/(.*))?$/);
  if (!m) return;
  auth(c);
  const p = s.projects.find(x => x.id === Number(m[1]));
  if (!p || !member(s, p, c.user!)) throw Object.assign(Error("Not found"), { status: 404, code: "not_found" });
  const tail = m[2] || "";
  if (!tail && c.req.method === "GET") return json(p);
  if (tail === "members" && c.req.method === "GET") {
    const members = [{ userId: p.ownerId, role: "owner" },
      ...[...(s.memberships.get(p.id) || [])].sort((a, b) => a[0] - b[0])
        .map(([userId, role]) => ({ userId, role }))];
    return json(members);
  }
  if (tail === "members" && c.req.method === "POST") {
    if (p.ownerId !== c.user!.id) throw Object.assign(Error("Forbidden"), { status: 403, code: "forbidden" });
    validate(Number.isInteger(body.userId) && s.users.some(u => u.id === body.userId), "userId");
    validate(["member", "viewer"].includes(body.role), "role");
    const map = s.memberships.get(p.id) || new Map();
    if (p.ownerId === body.userId || map.has(body.userId))
      throw Object.assign(Error("Already a member"), { status: 409, code: "conflict" });
    map.set(body.userId, body.role); s.memberships.set(p.id, map);
    return json({ userId: body.userId, role: body.role }, 201);
  }
  const del = tail.match(/^members\/(\d+)$/);
  if (del && c.req.method === "DELETE") {
    if (p.ownerId !== c.user!.id) throw Object.assign(Error("Forbidden"), { status: 403, code: "forbidden" });
    const id = Number(del[1]), map = s.memberships.get(p.id)!;
    if (!map?.delete(id)) throw Object.assign(Error("Not found"), { status: 404, code: "not_found" });
    s.tasks.filter(t => t.projectId === p.id && t.assigneeId === id).forEach(t => {
      t.assigneeId = null; t.version++; t.updatedAt = iso(s);
    });
    return new Response(null, { status: 204 });
  }
}
