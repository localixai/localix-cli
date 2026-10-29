import { json, bad, fail, notFound, forbidden, stamp } from "./http";
export function projectsRoute(c: any): Response {
  const { db, path, method, body: b, uid } = c;
  if (!uid) return fail(401, "unauthorized", "Unauthorized");
  if (path === "/projects" && method === "POST") {
    if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) return bad("name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) return bad("key");
    if (db.projects.some((p: any) => p.key === b.key)) return fail(409, "conflict", "Key exists", "key");
    const p = { id: ++db.counters.project, name: b.name.trim(), key: b.key, ownerId: uid,
      createdAt: stamp(db), members: [{ userId: uid, role: "owner" }] };
    db.projects.push(p); return json(clean(p), 201);
  }
  if (path === "/projects" && method === "GET")
    return json(db.projects.filter((p: any) => member(p, uid)).map(clean));
  const m = path.match(/^\/projects\/(\d+)(?:\/(.*))?$/);
  if (!m) return notFound();
  const p = db.projects.find((x: any) => x.id === Number(m[1]) && member(x, uid));
  if (!p) return notFound();
  const sub = m[2] || "";
  if (!sub && method === "GET") return json(clean(p));
  if (sub === "members" && method === "GET")
    return json([...p.members].sort((a: any, b: any) =>
      (a.role === "owner" ? -1 : b.role === "owner" ? 1 : a.userId - b.userId)));
  if (sub === "members" && method === "POST") {
    if (uid !== p.ownerId) return forbidden();
    if (!db.users.some((u: any) => u.id === b.userId)) return bad("userId");
    if (!["member", "viewer"].includes(b.role)) return bad("role");
    if (p.members.some((x: any) => x.userId === b.userId)) return fail(409, "conflict", "Already member");
    const item = { userId: b.userId, role: b.role }; p.members.push(item); return json(item, 201);
  }
  const rem = sub.match(/^members\/(\d+)$/);
  if (rem && method === "DELETE") {
    if (uid !== p.ownerId) return forbidden();
    const id = Number(rem[1]), ix = p.members.findIndex((x: any) => x.userId === id && id !== p.ownerId);
    if (ix < 0) return notFound();
    p.members.splice(ix, 1);
    db.tasks.forEach((t: any) => { if (t.projectId === p.id && t.assigneeId === id) t.assigneeId = null; });
    return json(null, 204);
  }
  return notFound();
}
export const member = (p: any, id: number) => p.members.some((m: any) => m.userId === id);
const clean = ({ members, ...p }: any) => p;
