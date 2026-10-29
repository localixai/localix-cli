import { State, err, ok, iso, projectFor } from "./model";
export async function projectRoutes(
  s: State,
  r: Request,
  path: string,
  m: string,
  u: number | undefined,
): Promise<Response | undefined> {
  const b = async (): Promise<any | null> => {
    try {
      const x = await r.json();
      return x && typeof x === "object" && !Array.isArray(x) ? x : null;
    } catch {
      return null;
    }
  };
  const list = path === "/projects";
  if (list && m === "POST") {
    if (u === undefined)
      return err("unauthorized", "Unauthorized", undefined, 401);
    const x = await b();
    if (!x) return err("validation", "Expected object", "body");
    if (
      typeof x.name !== "string" ||
      !x.name.trim() ||
      x.name.trim().length > 100
    )
      return err("validation", "Invalid name", "name");
    if (typeof x.key !== "string" || !/^[A-Z]{2,6}$/.test(x.key))
      return err("validation", "Invalid key", "key");
    if (s.projects.some((p) => p.key === x.key))
      return err("conflict", "Key taken", "key", 409);
    const p = {
      id: ++s.ids.project,
      key: x.key,
      name: x.name.trim(),
      ownerId: u,
      createdAt: iso(s),
      members: new Map([[u, "owner"]]),
      seq: 0,
    };
    s.projects.push(p);
    return ok(view(p), 201);
  }
  if (list && m === "GET") {
    if (u === undefined)
      return err("unauthorized", "Unauthorized", undefined, 401);
    return ok(s.projects.filter((p) => p.members.has(u)).map(view));
  }
  const match = path.match(
    /^\/projects\/([^/]+)(?:\/(members|stats|tasks(?:\/bulk)?))?(?:\/members\/([^/]+))?$/,
  );
  if (!match) return undefined;
  const id = Number(match[1]);
  if (!Number.isInteger(id))
    return err("not_found", "Not found", undefined, 404);
  const p = s.projects.find((x) => x.id === id);
  if (!p) return err("not_found", "Not found", undefined, 404);
  if (u === undefined)
    return err("unauthorized", "Unauthorized", undefined, 401);
  const role = p.members.get(u);
  if (!role) return err("not_found", "Not found", undefined, 404);
  if (!match[2] && m === "GET") return ok(view(p));
  if (match[2] === "members" && !match[3] && m === "GET")
    return ok([...p.members].sort(([a], [b]) => a - b).map(([userId, role]) => ({ userId, role })));
  if (match[2] === "members" && !match[3] && m === "POST") {
    if (role !== "owner") return err("forbidden", "Forbidden", undefined, 403);
    const x = await b();
    if (!x) return err("validation", "Expected object", "body");
    if (!["member", "viewer"].includes(x.role))
      return err("validation", "Invalid role", "role");
    if (!s.users.some((z) => z.id === x.userId))
      return err("validation", "Unknown user", "userId");
    if (p.members.has(x.userId))
      return err("conflict", "Already a member", undefined, 409);
    p.members.set(x.userId, x.role);
    return ok({ userId: x.userId, role: x.role }, 201);
  }
  if (match[3] && m === "DELETE") {
    if (role !== "owner") return err("forbidden", "Forbidden", undefined, 403);
    const uid = Number(match[3]);
    if (!p.members.has(uid) || uid === p.ownerId)
      return err("not_found", "Not found", undefined, 404);
    p.members.delete(uid);
    for (const t of s.tasks)
      if (t.projectId === id && t.assigneeId === uid) t.assigneeId = null;
    return new Response(null, { status: 204 });
  }
  if (match[2] === "stats" && m === "GET") {
    const ts = s.tasks.filter((t) => t.projectId === id);
    const count = (f: string) =>
      Object.fromEntries(
        ["open", "in_progress", "done"].map((x) => [
          x,
          ts.filter((t) => t[f as "status"] === x).length,
        ]),
      );
    const ass = new Map<number, number>();
    for (const t of ts)
      if (t.assigneeId !== null)
        ass.set(t.assigneeId, (ass.get(t.assigneeId) || 0) + 1);
    return ok({
      total: ts.length,
      byStatus: count("status"),
      byPriority: Object.fromEntries(
        ["high", "normal", "low"].map((x) => [
          x,
          ts.filter((t) => t.priority === x).length,
        ]),
      ),
      byAssignee: [...ass]
        .map(([userId, count]) => ({ userId, count }))
        .sort((a, b) => b.count - a.count || a.userId - b.userId),
    });
  }
  return undefined;
}
function view(p: any) {
  return {
    id: p.id,
    name: p.name,
    key: p.key,
    ownerId: p.ownerId,
    createdAt: p.createdAt,
  };
}
