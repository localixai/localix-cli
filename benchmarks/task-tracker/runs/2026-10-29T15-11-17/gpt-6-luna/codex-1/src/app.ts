import { err, fail, Fault, json, type State } from "./model";
import { users } from "./users";
import { projects } from "./projects";
import { tasks } from "./tasks";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const s: State = {
    users: [],
    projects: [],
    tasks: [],
    sessions: new Map(),
    idem: new Map(),
    hits: new Map(),
    counters: { user: 0, project: 0, task: 0, comment: 0 },
    now: options.now || Date.now,
  };
  return {
    async fetch(req) {
      const url = new URL(req.url),
        path = url.pathname,
        method = req.method;
      const allow = allowed(path);
      if (!allow)
        return json(
          { error: { code: "not_found", message: "not found" } },
          404,
        );
      if (!allow.includes(method))
        return json(
          {
            error: {
              code: "method_not_allowed",
              message: "method not allowed",
            },
          },
          405,
          { Allow: allow.join(", ") },
        );
      const token = req.headers
          .get("Authorization")
          ?.match(/^Bearer (.+)$/)?.[1],
        uid = token ? s.sessions.get(token) : undefined;
      if (token) {
        const arr = s.hits.get(token) || [],
          now = s.now(),
          fresh = arr.filter(
            (t) => Math.floor(t / 60000) === Math.floor(now / 60000),
          );
        if (fresh.length >= 100) {
          const retry = Math.ceil(
            ((Math.floor(now / 60000) + 1) * 60000 - now) / 1000,
          );
          return json(
            { error: { code: "rate_limited", message: "rate limited" } },
            429,
            {
              "Retry-After": String(retry),
              "X-RateLimit-Limit": "100",
              "X-RateLimit-Remaining": "0",
            },
          );
        }
        fresh.push(now);
        s.hits.set(token, fresh);
      }
      try {
        const answer = await users(req, path, s, uid);
        if (answer) return limited(answer, token, s);
        if (
          path === "/me" ||
          path === "/projects" ||
          path.startsWith("/projects/") ||
          path.startsWith("/tasks/") ||
          path === "/sessions"
        )
          if (!uid) fail(401, "unauthorized");
        const p = await projects(req, path, url, s, uid!);
        if (p) return limited(p, token, s);
        const t = await tasks(req, path, s, uid!);
        if (t) return limited(t, token, s);
        return json(
          { error: { code: "not_found", message: "not found" } },
          404,
        );
      } catch (e) {
        return limited(
          err(
            e instanceof Fault
              ? e
              : new Fault(500, "internal", "internal error"),
          ),
          token,
          s,
        );
      }
    },
  };
}
function limited(r: Response, token: string | undefined, s: State) {
  if (!token) return r;
  const headers = new Headers(r.headers),
    left = Math.max(0, 100 - (s.hits.get(token)?.length || 0));
  headers.set("X-RateLimit-Limit", "100");
  headers.set("X-RateLimit-Remaining", String(left));
  return new Response(r.body, { status: r.status, headers });
}
function allowed(path: string): string[] | undefined {
  if (path === "/users") return ["POST"];
  if (path === "/sessions") return ["POST", "DELETE"];
  if (path === "/me") return ["GET"];
  if (path === "/projects") return ["GET", "POST"];
  if (/^\/projects\/[^/]+$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/members$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/members\/\d+$/.test(path)) return ["DELETE"];
  if (/^\/projects\/\d+\/tasks$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/tasks\/bulk$/.test(path)) return ["POST"];
  if (/^\/projects\/\d+\/stats$/.test(path)) return ["GET"];
  if (/^\/tasks\/[^/]+$/.test(path)) return ["GET", "PATCH", "DELETE"];
  if (/^\/tasks\/[^/]+\/(transitions|dependencies)$/.test(path))
    return ["GET", "POST"];
  if (/^\/tasks\/[^/]+\/dependencies\/[^/]+$/.test(path)) return ["DELETE"];
  if (/^\/tasks\/[^/]+\/(history|comments)$/.test(path)) return ["GET", "POST"];
  return undefined;
}
