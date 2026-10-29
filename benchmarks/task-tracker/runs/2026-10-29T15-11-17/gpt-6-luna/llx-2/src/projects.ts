import type { Ctx } from "./types";
import { body, conflict, denied, json, member, missing, stamp, validation, visibleProject } from "./util";

export async function projects(ctx: Ctx): Promise<Response | null> {
  const { store, user, request: req, url } = ctx;
  if (!user) return null;
  if (url.pathname === "/projects" && req.method === "GET") {
    return json(store.projects.filter(p => p.members.has(user.id)).map(({ members, nextTask, ...p }) => p));
  }
  if (url.pathname === "/projects" && req.method === "POST") {
    const b = await body(req);
    if (!b) return validation("body");
    const name = typeof b.name === "string" ? b.name.trim() : "";
    if (!name || name.length > 100) return validation("name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) return validation("key");
    if (store.projects.some(p => p.key === b.key)) return conflict("key");
    const p = { id: store.projects.length + 1, name, key: b.key, ownerId: user.id,
      createdAt: stamp(ctx), members: new Map([[user.id, { userId: user.id, role: "owner" as const }]]),
      nextTask: 1 };
    store.projects.push(p);
    return json({ id: p.id, name, key: p.key, ownerId: user.id, createdAt: p.createdAt }, 201);
  }
  const match = url.pathname.match(/^\/projects\/([^/]+)(?:\/(members)(?:\/([^/]+))?)?$/);
  if (!match) return null;
  const id = /^\d+$/.test(match[1]!) ? Number(match[1]) : -1;
  const p = visibleProject(ctx, id);
  if (!p) return missing();
  const tail = match[2], person = match[3];
  if (!tail && req.method === "GET") {
    const { members, nextTask, ...reply } = p;
    return json(reply);
  }
  if (tail === "members" && !person && req.method === "GET") {
    return json([...p.members.values()].sort((a, b) =>
      a.role === "owner" ? -1 : b.role === "owner" ? 1 : a.userId - b.userId));
  }
  if (member(p, user.id)?.role !== "owner") return denied();
  if (tail === "members" && !person && req.method === "POST") {
    const b = await body(req);
    if (!b) return validation("body");
    if (!["member", "viewer"].includes(b.role)) return validation("role");
    if (!Number.isInteger(b.userId) || !store.users.some(u => u.id === b.userId)) return validation("userId");
    if (p.members.has(b.userId)) return conflict();
    p.members.set(b.userId, { userId: b.userId, role: b.role });
    return json({ userId: b.userId, role: b.role }, 201);
  }
  if (tail === "members" && person && req.method === "DELETE") {
    const uid = Number(person);
    if (!p.members.has(uid) || uid === p.ownerId) return missing();
    p.members.delete(uid);
    for (const t of store.tasks.filter(t => t.projectId === p.id && t.assigneeId === uid))
      t.assigneeId = null;
    return new Response(null, { status: 204 });
  }
  return null;
}
