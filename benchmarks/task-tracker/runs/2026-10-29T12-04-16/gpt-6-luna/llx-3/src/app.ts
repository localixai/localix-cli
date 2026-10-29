import type { State } from "./model";
import { answer, body, fail } from "./common";
import { userRoutes } from "./users";
import { taskRoutes } from "./tasks";
import { projectRoutes } from "./projects";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const s: State = { users: [], projects: [], members: new Map(), tasks: [],
    comments: new Map(), tokens: new Map(), limits: new Map(), idem: new Map(),
    sequences: new Map(),
    now: options.now ?? Date.now };
  return { async fetch(req) {
    const url = new URL(req.url), path = url.pathname, method = req.method;
    const allowed: Record<string, string> = { "/users": "POST", "/sessions": "POST, DELETE",
      "/me": "GET", "/projects": "GET, POST" };
    const uidToken = req.headers.get("Authorization")?.match(/^Bearer (.+)$/)?.[1];
    const uid = uidToken ? s.tokens.get(uidToken) : undefined;
    if (allowed[path] && !allowed[path].split(", ").includes(method))
      return answer({ error: { code: "method_not_allowed", message: "Method not allowed" } },
        405, { Allow: allowed[path] });
    if (path !== "/users" && path !== "/sessions" && !uid &&
      !["/me", "/projects"].includes(path) && !path.startsWith("/projects/"))
      return fail(404, "not_found");
    if (path.startsWith("/projects/") && !uid) return fail(401, "unauthorized");
    if (path === "/users" && method === "POST") {
      const r = userRoutes(s, method, path, await body(req), req); if (r) return r;
    }
    if (path === "/sessions" && ["POST", "DELETE"].includes(method)) {
      const r = userRoutes(s, method, path, await body(req), req); if (r) return r;
    }
    if (!uid) return fail(401, "unauthorized");
    if (uidToken) {
      const minute = Math.floor(s.now() / 60000), prev = s.limits.get(uidToken);
      const count = prev?.minute === minute ? prev.count : 0;
      const headers = { "X-RateLimit-Limit": "100",
        "X-RateLimit-Remaining": String(Math.max(0, 99 - count)) };
      if (count >= 100) return answer({ error: { code: "rate_limited", message: "rate limited" } }, 429,
        { ...headers, "Retry-After": String(Math.ceil((60000 - s.now() % 60000) / 1000)) });
      s.limits.set(uidToken, { minute, count: count + 1 });
    }
    if (path === "/me" && method === "GET") {
      const u = s.users.find(x => x.id === uid)!;
      return answer({ id: u.id, email: u.email, name: u.name }, 200,
        { "X-RateLimit-Limit": "100",
          "X-RateLimit-Remaining": String(Math.max(0, 100 - (s.limits.get(uidToken!)?.count ?? 1))) });
    }
    if (path === "/sessions" || path === "/users") {
      const user = await userRoutes(s, method, path, await body(req), req);
      if (user) return user;
    }
    const project = await projectRoutes(s, method, path, uid,
      () => body(req));
    if (project) return project;
    const r = await taskRoutes(s, method, path, url, uid, req);
    return r ?? fail(404, "not_found");
  } };
}
