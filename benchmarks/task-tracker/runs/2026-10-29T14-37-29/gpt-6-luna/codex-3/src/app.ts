import { State, err } from "./model";
import { authRoutes } from "./auth";
import { projectRoutes } from "./projects";
import { taskRoutes } from "./tasks";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const s: State = {
    users: [],
    projects: [],
    tasks: [],
    sessions: new Map(),
    idem: new Map(),
    limits: new Map(),
    now: options.now ?? Date.now,
    ids: { user: 0, project: 0, task: 0, comment: 0 },
  };
  return {
    async fetch(r) {
      const path = new URL(r.url).pathname,
        m = r.method;
      let token = r.headers.get("Authorization") || "";
      const user = token.startsWith("Bearer ")
        ? s.sessions.get(token.slice(7))
        : undefined;
      const known = routeMethods(path);
      if (known && !known.includes(m))
        return new Response(
          JSON.stringify({
            error: {
              code: "method_not_allowed",
              message: "Method not allowed",
            },
          }),
          {
            status: 405,
            headers: {
              "Content-Type": "application/json",
              Allow: known.join(", "),
            },
          },
        );
      let limited: Response | undefined;
      if (user !== undefined) {
        const minute = Math.floor(s.now() / 60000),
          a = s.limits.get(token) || { minute, count: 0 };
        if (a.minute !== minute) {
          a.minute = minute;
          a.count = 0;
        }
        a.count++;
        s.limits.set(token, a);
        if (a.count > 100) {
          const wait = Math.ceil((60000 - (s.now() % 60000)) / 1000);
          limited = err("rate_limited", "Rate limited", undefined, 429);
          limited.headers.set("Retry-After", String(wait));
          limited.headers.set("X-RateLimit-Limit", "100");
          limited.headers.set("X-RateLimit-Remaining", "0");
        } else {
          const response = await dispatch(s, r, path, m, user);
          response.headers.set("X-RateLimit-Limit", "100");
          response.headers.set("X-RateLimit-Remaining", String(100 - a.count));
          return response;
        }
      }
      if (limited) return limited;
      return dispatch(s, r, path, m, user);
    },
  };
}
async function dispatch(
  s: State,
  r: Request,
  p: string,
  m: string,
  u: number | undefined,
) {
  return (
    (await authRoutes(s, r, p, m, u)) ??
    (await projectRoutes(s, r, p, m, u)) ??
    (await taskRoutes(s, r, p, m, u)) ??
    err("not_found", "Not found", undefined, 404)
  );
}
function routeMethods(p: string): string[] | undefined {
  if (p === "/users" || p === "/sessions")
    return p === "/users" ? ["POST"] : ["POST", "DELETE"];
  if (p === "/me") return ["GET"];
  if (p === "/projects") return ["GET", "POST"];
  if (/^\/projects\/\d+\/tasks(?:\/bulk)?$/.test(p))
    return p.endsWith("/bulk") ? ["POST"] : ["GET", "POST"];
  if (/^\/projects\/\d+$/.test(p)) return ["GET"];
  if (/^\/projects\/\d+\/members$/.test(p)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/members\/\d+$/.test(p)) return ["DELETE"];
  if (/^\/projects\/\d+\/stats$/.test(p)) return ["GET"];
  if (/^\/tasks\/[^/]+$/.test(p)) return ["GET", "PATCH", "DELETE"];
  if (
    /^\/tasks\/[^/]+\/(transitions|dependencies|comments|history)(?:\/[^/]+)?$/.test(
      p,
    )
  ) {
    const sub = p.split("/")[3];
    if (sub === "history") return ["GET"];
    if (sub === "transitions") return ["POST"];
    if (sub === "comments") return ["GET", "POST"];
    return p.split("/").length === 5 ? ["DELETE"] : ["GET", "POST"];
  }
  return undefined;
}
