import type { State } from "./types";
import { Fault, fail, valid } from "./util";
import { users } from "./users";
import { projects } from "./projects";
import { tasks } from "./tasks";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const s: State = { users: [], projects: [], tasks: [], tokens: new Map(), limits: new Map(),
    idem: new Map(), now: options.now ?? Date.now,
    ids: { user: 0, project: 0, task: 0, comment: 0 } };
  return { fetch: async (request) => {
    const token = (request.headers.get("Authorization") ?? "").replace(/^Bearer /, "");
    const response = await handle(request, s);
    const limit = s.limits.get(token);
    if (!limit || !token) return response;
    const headers = new Headers(response.headers);
    headers.set("X-RateLimit-Limit", "100");
    headers.set("X-RateLimit-Remaining", String(Math.max(0, 100 - limit.count)));
    return new Response(response.body, { status: response.status, headers });
  } };
}
async function handle(req: Request, s: State): Promise<Response> {
  const url = new URL(req.url), path = url.pathname, method = req.method;
  const allowed = methods(path);
  if (allowed && !allowed.includes(method)) return response({ error: {
    code: "method_not_allowed", message: "Method not allowed" } }, 405, { Allow: allowed.join(", ") });
  try {
    let body: any = {};
    if (["POST", "PATCH", "PUT"].includes(method) && req.body) {
      try { body = await req.json(); } catch { fail(400, "validation", "Invalid JSON"); }
      valid(body !== null && typeof body === "object" && !Array.isArray(body), "body");
    }
    const auth = req.headers.get("Authorization") ?? "";
    const token = auth.startsWith("Bearer ") ? auth.slice(7) : "";
    let uid = 0;
    if (path !== "/users" && path !== "/sessions") {
      uid = s.tokens.get(token) ?? 0;
      if (!uid && (methods(path) || path.startsWith("/tasks/"))) fail(401, "unauthorized");
      if (!uid) fail(404, "not_found");
      const minute = Math.floor(s.now() / 60000), limit = s.limits.get(token);
      const cur = limit?.minute === minute ? limit : { minute, count: 0 };
      if (cur.count >= 100) return response({ error: { code: "rate_limited", message: "Rate limited" } },
        429, { "Retry-After": String(Math.ceil((60000 - s.now() % 60000) / 1000)),
          "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "0" });
      cur.count++; s.limits.set(token, cur);
    }
    if (path === "/users" || path === "/sessions" || path === "/me")
      return users(s, method, path, body, uid, token);
    if (path === "/projects" || /^\/projects\/\d+(\/|$)/.test(path))
      return projects(s, method, path, body, uid, url, req.headers);
    if (path.startsWith("/tasks/")) return tasks(s, method, path, body, uid, req.headers, url);
    return fail(404, "not_found");
  } catch (e) {
    if (e instanceof Fault) return response({ error: { code: e.code, message: e.message,
      ...(e.field ? { field: e.field } : {}) } }, e.status);
    return response({ error: { code: "validation", message: "Invalid request" } }, 400);
  }
}
function response(body: unknown, status = 200, headers: Record<string, string> = {}): Response {
  if (status === 204) return new Response(null, { status, headers });
  return Response.json(body, { status, headers });
}
function methods(path: string): string[] | null {
  if (["/users", "/sessions"].includes(path)) return ["POST", ...(path === "/sessions" ? ["DELETE"] : [])];
  if (path === "/me") return ["GET"];
  if (path === "/projects") return ["GET", "POST"];
  if (/^\/projects\/\d+$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/(tasks|tasks\/bulk|members|stats)$/.test(path))
    return path.endsWith("/stats") ? ["GET"] : ["GET", "POST"];
  if (/^\/projects\/\d+\/members\/\d+$/.test(path)) return ["DELETE"];
  if (/^\/tasks\/[^/]+$/.test(path)) return ["GET", "PATCH", "DELETE"];
  if (/^\/tasks\/[^/]+\/(comments|history|dependencies|transitions)$/.test(path))
    return path.endsWith("/history") ? ["GET"] : ["GET", "POST"];
  if (/^\/tasks\/[^/]+\/dependencies\/[^/]+$/.test(path)) return ["DELETE"];
  return null;
}
