import type { DB } from "./state";
import { json, fail, bad, notFound, forbidden, bodyObject } from "./http";
import { usersRoute } from "./users";
import { projectsRoute } from "./projects";
import { tasksRoute } from "./tasks";

export async function handle(req: Request, db: DB): Promise<Response> {
  const url = new URL(req.url), path = url.pathname, method = req.method;
  const token = req.headers.get("Authorization")?.match(/^Bearer (.+)$/)?.[1];
  const uid = token ? db.tokens.get(token) : undefined;
  if (token && uid !== undefined) {
    const min = Math.floor(db.now() / 60000);
    let l = db.limits.get(token);
    if (!l || l.minute !== min) l = { minute: min, count: 0 };
    if (l.count >= 100) {
      const r = fail(429, "rate_limited", "Rate limited");
      r.headers.set("Retry-After", String(Math.ceil(((min + 1) * 60000 - db.now()) / 1000)));
      r.headers.set("X-RateLimit-Limit", "100"); r.headers.set("X-RateLimit-Remaining", "0");
      return r;
    }
    l.count++; db.limits.set(token, l);
  }
  let value: any = {};
  if (["POST", "PATCH", "PUT"].includes(method) && req.headers.has("Content-Type")) {
    try { value = await req.json(); } catch { return fail(400, "validation", "Invalid JSON"); }
    if (!bodyObject(value)) return fail(400, "validation", "Expected object");
  }
  const ctx = { req, db, url, path, method, uid, body: value };
  if (path === "/users" || path === "/sessions" || path === "/me") {
    const response = usersRoute(ctx);
    if (token && uid !== undefined && path === "/me") {
      const l = db.limits.get(token)!;
      response.headers.set("X-RateLimit-Limit", "100");
      response.headers.set("X-RateLimit-Remaining", String(100 - l.count));
    }
    return response;
  }
  if (path === "/projects" && method !== "GET" && method !== "POST") {
    const r = fail(405, "method_not_allowed", "Method not allowed");
    r.headers.set("Allow", "GET, POST");
    return r;
  }
  if (/^\/projects\/\d+\/(tasks|stats)/.test(path) || path.startsWith("/tasks"))
    return tasksRoute(ctx);
  if (path.startsWith("/projects")) return projectsRoute(ctx);
  if (path === "/users" && method !== "POST") {
    const r = fail(405, "method_not_allowed", "Method not allowed");
    r.headers.set("Allow", "POST");
    return r;
  }
  return notFound();
}
