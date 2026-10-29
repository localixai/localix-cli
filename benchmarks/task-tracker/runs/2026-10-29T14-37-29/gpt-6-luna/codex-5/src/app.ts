import { auth, body, err, type Ctx, type DB } from "./model";
import { users } from "./users";
import { projects } from "./projects";
import { tasks } from "./tasks";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const db: DB = {
    users: [],
    projects: [],
    tasks: [],
    sessions: new Map(),
    members: new Map(),
    idem: new Map(),
    limits: new Map(),
  };
  const routes: Record<string, string[]> = {
    "/users": ["POST"],
    "/sessions": ["POST", "DELETE"],
    "/me": ["GET"],
    "/projects": ["GET", "POST"],
  };
  return {
    async fetch(req) {
      const url = new URL(req.url),
        path = url.pathname,
        method = req.method;
      const route =
        path.match(
          /^\/projects\/\d+(?:\/tasks(?:\/bulk)?|\/members(?:\/\d+)?|\/stats)?$/,
        ) ||
        path.match(
          /^\/tasks\/[^/]+(?:\/(?:comments|history|dependencies(?:\/[^/]+)?|transitions))?$/,
        );
      let allow = routes[path];
      if (route) {
        const suffix = path.match(/^\/projects\/\d+\/tasks(?:\/bulk)?$/)
          ? "project-tasks"
          : path.includes("/tasks")
            ? "tasks"
            : path.includes("/members")
              ? "members"
              : path.includes("/stats")
                ? "stats"
                : null;
        allow =
          suffix === "project-tasks"
            ? ["GET", "POST"]
            : suffix === "members"
              ? path.endsWith("/members")
                ? ["GET", "POST"]
                : ["DELETE"]
              : suffix === "stats"
                ? ["GET"]
                : path.includes("/transitions") ||
                    path.includes("/dependencies")
                  ? ["GET", "POST", "DELETE"]
                  : path.includes("/comments")
                    ? ["GET", "POST"]
                    : path.includes("/history")
                      ? ["GET"]
                      : ["GET", "PATCH", "DELETE"];
      }
      if (allow && !allow.includes(method))
        return err("method_not_allowed", 405, undefined, {
          Allow: (allow || []).join(", "),
        });
      const c: Ctx = { db, now: options.now || Date.now, user: null, req, url };
      if (method === "POST" && ["/users", "/sessions"].includes(path))
        return (await users(c, path, method)) as Response;
      if (path === "/users" || path === "/sessions") {
        if (method === "DELETE" && path === "/sessions") {
          c.user = auth(c);
          return (await users(c, path, method)) as Response;
        }
        return err("method_not_allowed", 405);
      }
      c.user = auth(c);
      if (!c.user) {
        if (!route && !routes[path]) return err("not_found", 404);
        return err("unauthorized", 401);
      }
      const token =
          req.headers.get("Authorization")?.replace(/^Bearer /, "") || "",
        minute = Math.floor(c.now() / 60000),
        hit = db.limits.get(token) || { minute, count: 0 };
      if (hit.minute !== minute) {
        hit.minute = minute;
        hit.count = 0;
      }
      hit.count++;
      db.limits.set(token, hit);
      const remaining = Math.max(0, 100 - hit.count),
        headers = {
          "X-RateLimit-Limit": "100",
          "X-RateLimit-Remaining": String(remaining),
        };
      if (hit.count > 100)
        return Response.json(
          { error: { code: "rate_limited", message: "rate limited" } },
          {
            status: 429,
            headers: {
              ...headers,
              "Retry-After": String(
                Math.ceil((60000 - (c.now() % 60000)) / 1000),
              ),
            },
          },
        );
      const res =
        (await users(c, path, method)) ||
        (await projects(c, path, method)) ||
        (await tasks(c, path, method));
      if (res) {
        const h = new Headers(res.headers);
        h.set("X-RateLimit-Limit", "100");
        h.set("X-RateLimit-Remaining", String(remaining));
        return new Response(res.body, { status: res.status, headers: h });
      }
      return err(route ? "not_found" : "not_found", 404);
    },
  };
}
