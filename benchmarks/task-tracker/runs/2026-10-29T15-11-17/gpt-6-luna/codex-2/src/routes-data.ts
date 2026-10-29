import { err, iso, key, ok, type Project, type State } from "./model";
import { createTasks, listTasks } from "./task-input";
import { taskRoute } from "./routes-tasks";
const num = (v: string) => (/^\d+$/.test(v) ? Number(v) : NaN);
const projectOf = (s: State, id: number) => s.projects.find((p) => p.id === id);
const role = (s: State, p: Project, u: number) =>
  u === p.ownerId ? "owner" : s.memberships.get(key(p.id, u));
const member = (s: State, p: Project, u: number) => !!role(s, p, u);
export async function dataRoute(
  s: State,
  m: string,
  path: string,
  url: URL,
  b: any,
  u: number | undefined,
  token: string,
  headers: Headers,
): Promise<Response | null> {
  const requireUser = () => (u ? null : err("unauthorized", "unauthorized", 401));
  if (path === "/projects" && m === "GET") {
    const a = requireUser();
    if (a) return a;
    return ok(s.projects.filter((p) => member(s, p, u!)).map(({ next, ...p }) => p));
  }
  if (path === "/projects" && m === "POST") {
    const a = requireUser();
    if (a) return a;
    const name = typeof b.name === "string" ? b.name.trim() : "";
    if (!name || name.length > 100) return err("validation", "invalid name", 400, "name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key))
      return err("validation", "invalid key", 400, "key");
    if (s.projects.some((p) => p.key === b.key)) return err("conflict", "key exists", 409, "key");
    const p = {
      id: s.projects.length + 1,
      name,
      key: b.key,
      ownerId: u!,
      createdAt: iso(s),
      next: 1,
    };
    s.projects.push(p);
    const { next: _next, ...view } = p;
    return ok(view, 201);
  }
  let match = path.match(/^\/projects\/([^/]+)(?:\/(.*))?$/);
  if (match) {
    const id = num(match[1]!),
      p = projectOf(s, id);
    const sub = match[2] ?? "";
    if (sub === "" && m === "GET") {
      const a = requireUser();
      if (a) return a;
      if (!p || !member(s, p, u!)) return err("not_found", "not found", 404);
      const { next: _next, ...view } = p;
      return ok(view);
    }
    if (!p) return err("not_found", "not found", 404);
    if (sub === "members" && m === "GET") {
      if (!u) return requireUser()!;
      if (!member(s, p, u)) return err("not_found", "not found", 404);
      return ok([
        { userId: p.ownerId, role: "owner" },
        ...[...s.memberships]
          .filter(([k]) => k.startsWith(p.id + ":"))
          .map(([k, r]) => ({ userId: Number(k.split(":")[1]), role: r }))
          .sort((a, b) => a.userId - b.userId),
      ]);
    }
    if (sub === "members" && m === "POST") {
      if (!u) return requireUser()!;
      if (!member(s, p, u)) return err("not_found", "not found", 404);
      if (p.ownerId !== u) return err("forbidden", "forbidden", 403);
      if (!["member", "viewer"].includes(b.role))
        return err("validation", "invalid role", 400, "role");
      if (!s.users.some((x) => x.id === b.userId))
        return err("validation", "unknown user", 400, "userId");
      if (member(s, p, b.userId)) return err("conflict", "already a member", 409);
      s.memberships.set(key(p.id, b.userId), b.role);
      return ok({ userId: b.userId, role: b.role }, 201);
    }
    const rm = sub.match(/^members\/(\d+)$/);
    if (rm && m === "DELETE") {
      if (!u) return requireUser()!;
      if (!member(s, p, u)) return err("not_found", "not found", 404);
      if (p.ownerId !== u) return err("forbidden", "forbidden", 403);
      const uid = Number(rm[1]);
      if (!s.memberships.delete(key(p.id, uid))) return err("not_found", "not found", 404);
      for (const t of s.tasks)
        if (t.projectId === p.id && t.assigneeId === uid) t.assigneeId = null;
      return new Response(null, { status: 204 });
    }
    if ((sub === "tasks" || sub === "tasks/bulk") && m === "POST")
      return createTasks(
        s,
        p,
        sub === "tasks/bulk" && !Array.isArray(b.tasks) ? { ...b, tasks: [] } : b,
        u,
        token,
        headers.get("Idempotency-Key") ?? "",
      );
    if (sub === "tasks" && m === "GET") return listTasks(s, p, url, u);
    if (sub === "stats" && m === "GET") {
      if (!u) return requireUser()!;
      if (!member(s, p, u)) return err("not_found", "not found", 404);
      const ts = s.tasks.filter((t) => t.projectId === p.id && !t.deleted),
        bucket = (field: string, values: string[]) =>
          Object.fromEntries(
            values.map((v) => [v, ts.filter((t) => (t as any)[field] === v).length]),
          );
      const ass = new Map<number, number>();
      for (const t of ts) if (t.assigneeId) ass.set(t.assigneeId, (ass.get(t.assigneeId) ?? 0) + 1);
      return ok({
        total: ts.length,
        byStatus: bucket("status", ["open", "in_progress", "done"]),
        byPriority: bucket("priority", ["high", "normal", "low"]),
        byAssignee: [...ass]
          .sort((a, b) => b[1] - a[1] || a[0] - b[0])
          .map(([userId, count]) => ({ userId, count })),
      });
    }
  }
  return taskRoute(s, m, path, b, u, token, headers);
}
function requestHeader(_url: URL) {
  return "";
}
