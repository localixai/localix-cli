import { bad, conflict, error, iso, json, member, type Context, type Project, type User } from "./shared";
import { tasksRoute } from "./tasks";

export type { Context } from "./shared";
async function body(req: Request): Promise<any> {
  try {
    const x = await req.json();
    return x && typeof x === "object" && !Array.isArray(x) ? x : null;
  } catch { return null; }
}
function auth(c: Context, req: Request) {
  const token = req.headers.get("Authorization")?.match(/^Bearer (.+)$/)?.[1];
  return token ? c.sessions.get(token) : undefined;
}
function validName(x: any) { return typeof x === "string" && !!x.trim() && x.trim().length <= 100; }
function projectOf(c: Context, n: string) { return c.projects.find(p => p.id === Number(n)); }
export async function route(c: Context, req: Request): Promise<Response> {
  const u = new URL(req.url), path = u.pathname.split("/").filter(Boolean), m = req.method;
  const allowed: Record<string, string> = { "/users": "POST", "/sessions": "POST, DELETE",
    "/projects": "GET, POST", "/me": "GET" };
  const allow = allowed[u.pathname] ?? (path[0] === "tasks" ? "GET, PATCH, DELETE" :
    path[0] === "projects" && path[2] === "members" ? "GET, POST" :
      path[0] === "projects" && (path[2] === "tasks" || path[2] === "stats") ?
        "GET, POST" :
      path[0] === "projects" && path.length === 2 ? "GET" : undefined);
  if (!allow) return error(404, "not_found", "Not found");
  const uid = auth(c, req);
  if (!allow.split(", ").includes(m) && !(path[0] === "tasks" && path[1] &&
      ["POST", "GET", "PATCH", "DELETE"].includes(m)) &&
      !(path[0] === "projects" && path.length > 2)) {
    const response = error(405, "method_not_allowed", "Method not allowed");
    response.headers.set("Allow", allow);
    return response;
  }
  if (path[0] === "users" && m === "POST") {
    const b = await body(req); if (!b) return error(400, "validation", "Invalid body");
    const email = typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
    if (!/^[^@]+@[^@]+\.[^@]+$/.test(email)) return bad("email");
    if (typeof b.password !== "string" || b.password.length < 8) return bad("password");
    if (!validName(b.name)) return bad("name");
    if (c.users.some(x => x.email === email)) return conflict("email");
    const x: User = { id: ++c.ids.user, email, password: b.password, name: b.name.trim() };
    c.users.push(x); return json({ id: x.id, email, name: x.name }, 201);
  }
  if (path[0] === "sessions" && m === "POST") {
    const b = await body(req);
    const user = c.users.find(x =>
      x.email === String(b?.email).toLowerCase() && x.password === b?.password);
    if (!user) return error(401, "unauthorized", "Unauthorized");
    const token = `token-${user.id}-${c.sessions.size + 1}-${c.now()}`; c.sessions.set(token, user.id);
    return json({ token }, 201);
  }
  if (path[0] === "sessions" && m === "DELETE") {
    if (!uid) return error(401, "unauthorized", "Unauthorized");
    const token = req.headers.get("Authorization")!.slice(7); c.sessions.delete(token);
    return new Response(null, { status: 204 });
  }
  if (!uid) return error(401, "unauthorized", "Unauthorized");
  const rate = req.headers.get("Authorization")?.slice(7) ?? "";
  const minute = Math.floor(c.now() / 60000), r = c.rates.get(rate);
  if (!r || r.minute !== minute) c.rates.set(rate, { minute, count: 1 });
  else if (r.count >= 100) return json({ error: { code: "rate_limited", message: "Rate limited" } }, 429,
    { "Retry-After": String(Math.ceil(((minute + 1) * 60000 - c.now()) / 1000)),
      "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "0" });
  else r.count++;
  if (path[0] === "me" && m === "GET") {
    const x = c.users.find(v => v.id === uid)!;
    return json({ id: x.id, email: x.email, name: x.name }, 200,
      { "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": String(100 - (c.rates.get(rate)?.count ?? 1)) });
  }
  if (path[0] === "projects" && path.length === 1 && m === "POST") {
    const b = await body(req); if (!b) return bad("body");
    if (!validName(b.name)) return bad("name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) return bad("key");
    if (c.projects.some(x => x.key === b.key)) return conflict("key");
    const p: Project = { id: ++c.ids.project, key: b.key, name: b.name.trim(), ownerId: uid, createdAt: iso(c) };
    c.projects.push(p); c.members.set(p.id, new Map()); return json(p, 201);
  }
  if (path[0] === "projects" && path.length === 1 && m === "GET")
    return json(c.projects.filter(p => member(c, p, uid)).sort((a,b) => a.id-b.id));
  if (path[0] === "projects" && path.length > 1) {
    const p = projectOf(c, path[1]!); if (!p || !member(c, p, uid)) return error(404, "not_found", "Not found");
    if (path.length === 2 && m === "GET") return json(p);
    if (path[2] === "members") {
      const map = c.members.get(p.id)!;
      if (path.length === 3 && m === "GET") return json([{ userId: p.ownerId, role: "owner" },
        ...[...map].sort((a,b)=>a[0]-b[0]).map(([userId, role]) => ({ userId, role }))]);
      if (path.length === 3 && m === "POST") {
        if (p.ownerId !== uid) return error(403, "forbidden", "Forbidden");
        const b = await body(req); if (!b) return bad("body");
        if (!["member", "viewer"].includes(b.role)) return bad("role");
        if (!c.users.some(x => x.id === b.userId)) return bad("userId");
        if (member(c, p, b.userId)) return conflict("userId");
        map.set(b.userId, b.role); return json({ userId: b.userId, role: b.role }, 201);
      }
      if (path.length === 4 && m === "DELETE") {
        const id = Number(path[3]); if (p.ownerId !== uid) return error(403, "forbidden", "Forbidden");
        if (!map.delete(id)) return error(404, "not_found", "Not found");
        c.tasks.filter(t => t.projectId === p.id && t.assigneeId === id).forEach(t => t.assigneeId = null);
        return new Response(null, { status: 204 });
      }
    }
    if (path[2] === "tasks" || path[2] === "stats") return tasksRoute(c, req, p, uid, path.slice(2), u);
  }
  if (path[0] === "tasks") return tasksRoute(c, req, undefined, uid, path.slice(1), u);
  return error(404, "not_found", "Not found");
}
