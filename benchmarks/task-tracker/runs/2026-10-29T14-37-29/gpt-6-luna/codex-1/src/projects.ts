import { bodyOf, fail, invalid, response } from "./errors";
import { member, type Context, type Project } from "./model";
import { requireUser } from "./auth";

export async function projectRoute(ctx: Context, req: Request, url: URL, method: string) {
  const path = url.pathname;
  const user = requireUser(ctx, req);
  if (path === "/projects" && method === "GET") {
    return response(ctx.state.projects.filter((p) => member(ctx, p.id, user.id)));
  }
  if (path === "/projects" && method === "POST") {
    const b = await bodyOf(req);
    if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) invalid("name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) invalid("key");
    if (ctx.state.projects.some((p) => p.key === b.key)) fail(409, "conflict", "Key already exists", "key");
    const p: Project = { id: ctx.state.nextProject++, name: b.name.trim(), key: b.key,
      ownerId: user.id, createdAt: new Date(ctx.now()).toISOString() };
    ctx.state.projects.push(p);
    ctx.state.memberships.set(p.id, new Map([[user.id, "owner"]]));
    return response(p, 201);
  }
  const m = path.match(/^\/projects\/(\d+)(?:\/(members)(?:\/(\d+))?)?$/);
  if (!m) return null;
  const p = ctx.state.projects.find((x) => x.id === Number(m[1]));
  if (!p || !member(ctx, p.id, user.id)) fail(404, "not_found", "Project not found");
  if (!m[2] && method === "GET") return response(p);
  if (m[2] && !m[3] && method === "GET") {
    const entries = [...ctx.state.memberships.get(p.id)!.entries()];
    entries.sort(([a], [b]) => a === p.ownerId ? -1 : b === p.ownerId ? 1 : a - b);
    return response(entries.map(([userId, role]) => ({ userId, role })));
  }
  if (m[2] && !m[3] && method === "POST") {
    if (member(ctx, p.id, user.id) !== "owner") fail(403, "forbidden", "Owner required");
    const b = await bodyOf(req);
    if (!Number.isInteger(b.userId) || !ctx.state.users.some((u) => u.id === b.userId)) invalid("userId");
    if (b.role !== "viewer" && b.role !== "member") invalid("role");
    const members = ctx.state.memberships.get(p.id)!;
    if (members.has(b.userId)) fail(409, "conflict", "Already a member");
    members.set(b.userId, b.role);
    return response({ userId: b.userId, role: b.role }, 201);
  }
  if (m[2] && m[3] && method === "DELETE") {
    if (member(ctx, p.id, user.id) !== "owner") fail(403, "forbidden", "Owner required");
    const uid = Number(m[3]);
    if (!ctx.state.memberships.get(p.id)!.has(uid) || uid === p.ownerId) fail(404, "not_found", "Member not found");
    ctx.state.memberships.get(p.id)!.delete(uid);
    for (const t of ctx.state.tasks.filter((x) => x.projectId === p.id && x.assigneeId === uid)) t.assigneeId = null;
    return response(null, 204);
  }
  return null;
}
