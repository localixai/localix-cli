import { state } from "./model";
import { auth, errorResponse, response } from "./http";
import { users } from "./users";
import { projects } from "./projects";
import { taskRoutes } from "./tasks";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const now = options.now || Date.now;
  const s = state();
  return { fetch: async (request: Request) => {
    try {
      const url = new URL(request.url), parts = url.pathname.split("/").filter(Boolean);
      const method = request.method.toUpperCase();
      if (!known(parts)) return response({ error: { code: "not_found", message: "Not found" } }, 404);
      if (!allowed(parts, method)) {
        return response({ error: { code: "method_not_allowed",
          message: "Method not allowed" } }, 405, { Allow: allow(parts) });
      }
      const token = request.headers.get("Authorization")?.replace(/^Bearer /, "");
      if (token && s.sessions.has(token)) {
        const minute = Math.floor(now() / 60000), old = s.limits.get(token);
        const count = old?.minute === minute ? old.count : 0;
        const headers = { "X-RateLimit-Limit": "100",
          "X-RateLimit-Remaining": String(Math.max(0, 99 - count)) };
        if (count >= 100) {
          const retry = Math.ceil(((minute + 1) * 60000 - now()) / 1000);
          return response({ error: { code: "rate_limited", message: "Rate limit exceeded" } }, 429,
            { ...headers, "Retry-After": String(retry) });
        }
        s.limits.set(token, { minute, count: count + 1 });
        const result = parts[0] === "users" || parts[0] === "sessions" || parts[0] === "me" ?
          await users(method, parts, request, s) : parts[0] === "projects" && parts[2] !== "tasks" ?
          await projects(method, parts, request, s, now) : await taskRoutes(method, parts, request, s, now, url);
        return addLimit(result, headers);
      }
      if (["users", "sessions"].includes(parts[0] || "") ||
        (parts[0] === "projects" && method === "POST" && parts.length === 1)) {
        return await usersOrProject(parts, method, request, s, now);
      }
      if (parts[0] === "users") return await users(method, parts, request, s);
      if (parts[0] === "sessions") return await users(method, parts, request, s);
      auth(request, s);
      return response(null, 401);
    } catch (e) { return errorResponse(e); }
  } };
}
function addLimit(r: Response, h: Record<string, string>): Response {
  const headers = new Headers(r.headers); Object.entries(h).forEach(([k, v]) => headers.set(k, v));
  return new Response(r.body, { status: r.status, headers });
}
async function usersOrProject(
  parts: string[], method: string, req: Request, s: ReturnType<typeof state>,
  now: () => number,
) {
  if (parts[0] === "projects") return projects(method, parts, req, s, now);
  return users(method, parts, req, s);
}
function known(p: string[]): boolean {
  return ["users", "sessions", "me", "projects", "tasks"].includes(p[0] || "");
}
function allowed(p: string[], m: string): boolean {
  return allow(p).split(", ").includes(m);
}
function allow(p: string[]): string {
  if (p[0] === "users") return "POST";
  if (p[0] === "sessions") return "POST, DELETE";
  if (p[0] === "me") return "GET";
  if (p[0] === "projects" && p.length === 1) return "GET, POST";
  if (p[0] === "projects" && p[2] === "tasks") return p[3] === "bulk" ? "POST" : "GET, POST";
  if (p[0] === "projects" && p[2] === "members") return "GET, POST, DELETE";
  if (p[0] === "projects") return "GET";
  if (p[0] === "tasks") return "GET, PATCH, DELETE, POST";
  return "GET";
}
