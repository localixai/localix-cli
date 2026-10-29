import { Context, State, fail, validateBody } from "./model";
import { userRoutes } from "./users";
import { projectRoutes } from "./projects";
import { taskRoutes } from "./tasks";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
const methods: Record<string, string[]> = {
  "/users": ["POST"],
  "/sessions": ["POST", "DELETE"],
  "/me": ["GET"],
  "/projects": ["GET", "POST"],
};
export function createApp(options: Options = {}): App {
  const state: State = {
    users: [],
    projects: [],
    tasks: [],
    sessions: new Map(),
    members: new Map(),
    histories: new Map(),
    comments: new Map(),
    deps: new Map(),
    counters: {},
    idempotency: new Map(),
    limits: new Map(),
  };
  const now = options.now ?? Date.now;
  return {
    async fetch(request: Request) {
      const url = new URL(request.url),
        path = url.pathname,
        method = request.method;
      let allow = methods[path];
      if (path.startsWith("/projects/")) allow = ["GET", "POST", "DELETE"];
      if (path.startsWith("/tasks/"))
        allow = ["GET", "POST", "PATCH", "DELETE"];
      const known =
        path === "/users" ||
        path === "/sessions" ||
        path === "/me" ||
        path === "/projects" ||
        /^\/projects\/\d+(?:\/(?:members(?:\/\d+)?|tasks(?:\/bulk)?|stats))?$/.test(
          path,
        ) ||
        /^\/tasks\/[A-Z]+-\d+(?:\/(?:transitions|history|comments|dependencies(?:\/[A-Z]+-\d+)?))?$/.test(
          path,
        );
      if (!known) return response(fail("not_found", "not found", 404));
      if (allow && !allow.includes(method))
        return response(fail("method_not_allowed", "method not allowed", 405), {
          Allow: allow.join(", "),
        });
      const auth = request.headers.get("Authorization") ?? "",
        token = auth.startsWith("Bearer ") ? auth.slice(7) : "";
      const userId = state.sessions.get(token);
      const protectedPath = path !== "/users" && path !== "/sessions";
      if (protectedPath && userId === undefined)
        return response(fail("unauthorized", "authentication required", 401));
      let extra: Record<string, string> = {};
      if (userId !== undefined) {
        const nowMs = now(),
          minute = Math.floor(nowMs / 60000),
          limit = state.limits.get(token);
        if (!limit || limit.minute !== minute)
          state.limits.set(token, { minute, count: 0 });
        const current = state.limits.get(token)!;
        if (current.count >= 100)
          return response(fail("rate_limited", "rate limit exceeded", 429), {
            "Retry-After": String(
              Math.ceil(((minute + 1) * 60000 - nowMs) / 1000),
            ),
            "X-RateLimit-Limit": "100",
            "X-RateLimit-Remaining": "0",
          });
        current.count++;
        extra = {
          "X-RateLimit-Limit": "100",
          "X-RateLimit-Remaining": String(100 - current.count),
        };
      }
      let body: any = {};
      if (["POST", "PATCH", "PUT"].includes(method)) {
        try {
          body = await request.json();
        } catch {
          return response(fail("validation", "invalid JSON body", 400));
        }
        if (!validateBody(body))
          return response(fail("validation", "body must be an object", 400));
      }
      const c: Context = { state, now, request, userId };
      let result =
        userRoutes(c, path, method, body) ??
        projectRoutes(c, path, method, body) ??
        taskRoutes(c, path, method, body, url);
      result ??= fail("not_found", "not found", 404);
      return response(result, extra);
    },
  };
}
function response(result: any, extra: Record<string, string> = {}) {
  const headers = new Headers({
    "Content-Type": "application/json",
    ...extra,
    ...result.headers,
  });
  return new Response(
    result.status === 204 ? null : JSON.stringify(result.body ?? {}),
    { status: result.status ?? 200, headers },
  );
}
