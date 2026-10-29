import type { State } from "./state";
import { err, auth } from "./util";
import { usersRoute } from "./users";
import { projectsRoute } from "./projects";
import { tasksRoute } from "./tasks";

export async function route(req: Request, s: State): Promise<Response> {
  const url = new URL(req.url), path = url.pathname;
  const methods: Record<string, string> = {
    "/users": "POST", "/sessions": "POST, DELETE", "/me": "GET",
    "/projects": "GET, POST",
  };
  const collection = path.match(/^\/projects\/\d+\/(members|tasks|stats)$/);
  const task = path.match(/^\/tasks\/[^/]+/);
  if (methods[path] && !methods[path].split(", ").includes(req.method)) {
    const response = err(405, "method_not_allowed", "Method not allowed");
    response.headers.set("Allow", methods[path]);
    return response;
  }
  const token = auth(req, s);
  const guarded = path !== "/users" && path !== "/sessions" && path !== "/nowhere";
  if (guarded && !token) return err(401, "unauthorized", "Authentication required");
  if (token) {
    const tokenValue = /^Bearer (.+)$/.exec(req.headers.get("Authorization") ?? "")![1]!;
    const minute = Math.floor(s.now() / 60000);
    let r = s.rates.get(tokenValue);
    if (!r || r.minute !== minute) r = { minute, count: 0 };
    r.count++;
    s.rates.set(tokenValue, r);
    const remaining = Math.max(0, 100 - r.count);
    if (r.count > 100) {
      const response = err(429, "rate_limited", "Rate limit exceeded");
      response.headers.set("Retry-After", String(Math.ceil(((minute + 1) * 60000 - s.now()) / 1000)));
      response.headers.set("X-RateLimit-Limit", "100");
      response.headers.set("X-RateLimit-Remaining", "0");
      return response;
    }
    // Apply rate headers to successful/error responses below.
    const response = await dispatch(req, s, token, url);
    response.headers.set("X-RateLimit-Limit", "100");
    response.headers.set("X-RateLimit-Remaining", String(remaining));
    return response;
  }
  return dispatch(req, s, token, url);
}

async function dispatch(req: Request, s: State, u: any, url: URL) {
  const a = await usersRoute(req, s, u, url.pathname);
  if (a) return a;
  const b = await projectsRoute(req, s, u, url);
  if (b) return b;
  const c = await tasksRoute(req, s, u, url);
  return c ?? err(404, "not_found", "Not found");
}
