import { projectRoute } from "./projects";
import { auth, failure, failureFor, readBody } from "./response";
import { taskRoute } from "./tasks";
import { userRoute } from "./users";
import type { State } from "./model";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const s: State = {
    users: [], projects: [], memberships: new Map(), tasks: [], comments: new Map(),
    history: new Map(), tokens: new Map(), limits: new Map(), now: options.now || Date.now,
    ids: { user: 0, project: 0, task: 0, comment: 0 }, deps: new Map(), idem: new Map(),
  };
  return { async fetch(req) {
    const url = new URL(req.url), path = url.pathname;
    try {
      const body = ["POST", "PATCH"].includes(req.method) ? await readBody(req) : {};
      const ctx = { s, req, url, body };
      const routes: Record<string, string[]> = {
        "/users": ["POST"], "/sessions": ["POST", "DELETE"], "/me": ["GET"],
        "/projects": ["GET", "POST"],
      };
      if (routes[path] && !routes[path].includes(req.method)) {
        const r = failure(405, "method_not_allowed", "Method not allowed");
        r.headers.set("Allow", routes[path].join(", "));
        return r;
      }
      let response = userRoute(ctx, path) || projectRoute(ctx, path) ||
        taskRoute(ctx, path) || failure(404, "not_found", "Not found");
      const token = req.headers.get("Authorization")?.match(/^Bearer (.+)$/)?.[1];
      const bucket = token && s.limits.get(token);
      if (bucket && bucket.minute === Math.floor(s.now() / 60000)) {
        response.headers.set("X-RateLimit-Limit", "100");
        response.headers.set("X-RateLimit-Remaining", String(Math.max(0, 100 - bucket.count)));
      }
      return response;
    } catch (e: any) {
      const response = failureFor(e);
      if (e.retry) {
        response.headers.set("Retry-After", String(e.retry));
        response.headers.set("X-RateLimit-Limit", "100");
        response.headers.set("X-RateLimit-Remaining", "0");
      }
      return response;
    }
  } };
}
