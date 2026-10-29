import { basic } from "./basic";
import { bad, err, object, readBody, response } from "./http";
import type { State } from "./model";
import { stats } from "./list";
import { taskRoutes } from "./tasks";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const s: State = {
    users: [],
    projects: [],
    tasks: [],
    sessions: new Map(),
    members: new Map(),
    limits: new Map(),
    idem: new Map(),
    now: options.now || Date.now,
    ids: { user: 0, project: 0, task: 0, comment: 0 },
  };
  return {
    async fetch(req) {
      const url = new URL(req.url),
        path = url.pathname,
        method = req.method.toUpperCase();
      const allowed = allowedMethods(path);
      if (allowed && !allowed.includes(method))
        return response(err(405, "method_not_allowed", "Method not allowed"), {
          Allow: allowed.join(", "),
        });
      if (!allowed) return response(err(404, "not_found", "Not found"));
      let user: number | null = null;
      const auth = req.headers.get("Authorization");
      if (auth?.startsWith("Bearer "))
        user = s.sessions.get(auth.slice(7)) ?? null;
      const needsAuth = !["POST /users", "POST /sessions"].includes(
        `${method} ${path}`,
      );
      if (needsAuth && user === null)
        return response(err(401, "unauthorized", "Unauthorized"));
      const extras: Record<string, string> = {};
      if (user !== null) {
        const token = auth!.slice(7),
          minute = Math.floor(s.now() / 60000),
          entry = s.limits.get(token);
        if (!entry || entry.minute !== minute)
          s.limits.set(token, { minute, count: 1 });
        else entry.count++;
        const count = s.limits.get(token)!.count;
        extras["X-RateLimit-Limit"] = "100";
        extras["X-RateLimit-Remaining"] = String(Math.max(0, 100 - count));
        if (count > 100) {
          extras["Retry-After"] = String(
            Math.ceil(((minute + 1) * 60000 - s.now()) / 1000),
          );
          return response(err(429, "rate_limited", "Rate limited"), extras);
        }
      }
      let body: any = {};
      if (["POST", "PATCH", "PUT"].includes(method)) {
        const raw = await req.text();
        body = readBody(raw);
        if (body === null) return response(bad(), extras);
      }
      const c = { s, req, user, url, body };
      let result = basic(c, path, method);
      if (
        result === undefined &&
        path.startsWith("/projects/") &&
        path.endsWith("/stats") &&
        method === "GET"
      ) {
        const id = Number(path.split("/")[2]),
          p = s.projects.find((x) => x.id === id);
        result =
          p && (user === p.ownerId || s.members.get(id)?.has(user!))
            ? stats(c, id)
            : {
                status: 404,
                body: { error: { code: "not_found", message: "Not found" } },
              };
      }
      if (result === undefined) result = taskRoutes(c, path, method);
      if (result === undefined) {
        const m = path.match(/^\/projects\/(\d+)\/members\/(\d+)$/);
        if (m && method === "DELETE") {
          const p = s.projects.find((x) => x.id === Number(m[1])),
            uid = Number(m[2]);
          if (!p || p.ownerId !== user)
            result = {
              status: 404,
              body: { error: { code: "not_found", message: "Not found" } },
            };
          else if (!s.members.get(p.id)?.has(uid))
            result = {
              status: 404,
              body: { error: { code: "not_found", message: "Not found" } },
            };
          else {
            s.members.get(p.id)!.delete(uid);
            for (const t of s.tasks)
              if (t.projectId === p.id && t.assigneeId === uid)
                t.assigneeId = null;
            result = { status: 204 };
          }
        }
      }
      return response(
        result || {
          status: 404,
          body: { error: { code: "not_found", message: "Not found" } },
        },
        extras,
      );
    },
  };
}
function allowedMethods(path: string): string[] | undefined {
  if (path === "/users" || path === "/sessions")
    return path === "/users" ? ["POST"] : ["POST", "DELETE"];
  if (path === "/me") return ["GET"];
  if (path === "/projects") return ["GET", "POST"];
  if (/^\/projects\/\d+$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/members$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/members\/\d+$/.test(path)) return ["DELETE"];
  if (/^\/projects\/\d+\/stats$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/tasks(\/bulk)?$/.test(path)) return ["GET", "POST"];
  if (/^\/tasks\/[^/]+$/.test(path)) return ["GET", "PATCH", "DELETE"];
  if (/^\/tasks\/[^/]+\/comments$/.test(path)) return ["GET", "POST"];
  if (/^\/tasks\/[^/]+\/(history|dependencies)$/.test(path))
    return ["GET", "POST"];
  if (/^\/tasks\/[^/]+\/transitions$/.test(path)) return ["POST"];
  if (/^\/tasks\/[^/]+\/dependencies\/[^/]+$/.test(path)) return ["DELETE"];
  return undefined;
}
