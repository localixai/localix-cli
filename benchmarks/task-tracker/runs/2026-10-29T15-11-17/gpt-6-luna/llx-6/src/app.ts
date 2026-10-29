import { readBody, fail, type Ctx } from "./http";
import { projectRoutes } from "./projects";
import { taskRoutes } from "./taskroutes";
import { userRoutes } from "./users";
import type { State } from "./model";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const state: State = {
    now: options.now ?? Date.now, users: [], projects: [], tasks: [],
    sessions: new Map(), limits: new Map(),
    ids: { user: 0, project: 0, task: 0, comment: 0 }, idem: new Map(),
  };
  return { fetch: async request => {
    const url = new URL(request.url), path = url.pathname;
    const methods: Record<string, string> = {
      "/users": "POST", "/sessions": "POST, DELETE", "/me": "GET",
      "/projects": "GET, POST",
    };
    const allowed = methods[path];
    const auth = request.headers.get("Authorization");
    const token = auth?.startsWith("Bearer ") ? auth.slice(7) : "";
    const user = state.sessions.get(token) ?? null;
    if (allowed && !allowed.split(", ").includes(request.method))
      return new Response(JSON.stringify({ error: { code: "method_not_allowed", message: "Method not allowed" } }),
        { status: 405, headers: { "Content-Type": "application/json", Allow: allowed } });
    const body = await readBody(request);
    const ctx: Ctx = { state, user, body, url, req: request };
    let result: any;
    const publicPath = path === "/users" || path === "/sessions";
    const known = path === "/me" || path === "/users" || path === "/sessions" ||
      path === "/projects" || path.startsWith("/projects/") || path.startsWith("/tasks/");
    if (!known) result = fail(404, "not_found");
    else if (!publicPath && !user) result = fail(401, "unauthorized");
    else if (!publicPath && user) {
      const minute = Math.floor(state.now() / 60000);
      let limit = state.limits.get(token);
      if (!limit || limit.minute !== minute) limit = { minute, count: 0 };
      if (limit.count >= 100) {
        const retry = Math.ceil(((minute + 1) * 60000 - state.now()) / 1000);
        state.limits.set(token, limit);
        result = fail(429, "rate_limited");
        return response(result, { "Retry-After": String(retry), "X-RateLimit-Limit": "100",
          "X-RateLimit-Remaining": "0" });
      }
      limit.count++; state.limits.set(token, limit);
      ctx.user = user;
    }
    if (!result) {
      if (body === null) result = fail(400, "validation", "Invalid JSON");
      else result = userRoutes(ctx, path) ?? projectRoutes(ctx, path) ?? taskRoutes(ctx, path) ??
        fail(404, "not_found");
    }
    const headers: Record<string, string> = { ...(result.headers ?? {}) };
    if (user) {
      const l = state.limits.get(token);
      headers["X-RateLimit-Limit"] = "100";
      headers["X-RateLimit-Remaining"] = String(Math.max(0, 100 - (l?.count ?? 0)));
    }
    return response(result, headers);
  } };
}
function response(result: any, headers: Record<string, string>): Response {
  const h = new Headers(headers);
  if (result.status !== 204) h.set("Content-Type", "application/json");
  const body = result.status === 204 ? null : JSON.stringify(result.body);
  return new Response(body, { status: result.status, headers: h });
}
