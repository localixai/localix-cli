import type { Ctx, Result } from "./types";
import { fail, iso, member, next, ok, projectAccess } from "./util";
export function projectRoute(c: Ctx, path: string, m: string): Result | null {
  const s = c.state,
    b = c.body;
  if (path === "/projects" && m === "GET")
    return ok(
      s.projects
        .filter((p) => projectAccess(c, p.id))
        .sort((a, b) => a.id - b.id),
    );
  if (path === "/projects" && m === "POST") {
    if (
      typeof b?.name !== "string" ||
      !b.name.trim() ||
      b.name.trim().length > 100
    )
      return fail(400, "validation", "invalid name", "name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key))
      return fail(400, "validation", "invalid key", "key");
    if (s.projects.some((p) => p.key === b.key))
      return fail(409, "conflict", "key already used", "key");
    const p = {
      id: next(c, "project"),
      name: b.name.trim(),
      key: b.key,
      ownerId: c.user!.id,
      createdAt: iso(c),
    };
    s.projects.push(p);
    s.members.set(p.id, new Map([[p.ownerId, "owner"]]));
    return ok(p, 201);
  }
  let x = path.match(
    /^\/projects\/(\d+)(?:\/(members|tasks|stats)(?:\/(\d+))?)?$/,
  );
  if (!x) return null;
  const id = Number(x[1]),
    sub = x[2],
    p = s.projects.find((q) => q.id === id);
  if (!p || !member(c, id)) return fail(404, "not_found");
  if (!sub && m === "GET") return ok(p);
  if (sub === "members") {
    const map = s.members.get(id)!;
    const target = x[3] ? Number(x[3]) : undefined;
    if (m === "GET" && !target)
      return ok(
        [...map]
          .map(([userId, role]) => ({ userId, role }))
          .sort((a, b) =>
            a.role === "owner"
              ? -1
              : b.role === "owner"
                ? 1
                : a.userId - b.userId,
          ),
      );
    if (m === "POST") {
      if (p.ownerId !== c.user!.id) return fail(403, "forbidden");
      if (!["member", "viewer"].includes(b?.role))
        return fail(400, "validation", "invalid role", "role");
      if (!s.users.some((u) => u.id === b.userId))
        return fail(400, "validation", "unknown user", "userId");
      if (map.has(b.userId)) return fail(409, "conflict");
      map.set(b.userId, b.role);
      return ok({ userId: b.userId, role: b.role }, 201);
    }
    if (m === "DELETE" && target) {
      if (p.ownerId !== c.user!.id) return fail(403, "forbidden");
      if (!map.has(target) || target === p.ownerId)
        return fail(404, "not_found");
      map.delete(target);
      s.tasks
        .filter((t) => t.projectId === id && t.assigneeId === target)
        .forEach((t) => (t.assigneeId = null));
      return ok(undefined, 204);
    }
  }
  if (sub === "tasks" && m === "GET") return null;
  if (sub === "stats" && m === "GET") return null;
  return null;
}
