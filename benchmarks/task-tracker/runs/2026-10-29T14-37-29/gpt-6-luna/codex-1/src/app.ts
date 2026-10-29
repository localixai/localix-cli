import { authRoute, bearer } from "./auth";
import { ApiError, errorResponse, fail } from "./errors";
import { projectRoute } from "./projects";
import { taskRoute } from "./tasks";
import { type Context, type State } from "./model";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
function state(): State {
  return { users: [], projects: [], tasks: [], memberships: new Map(), sessions: new Map(),
    histories: new Map(), comments: new Map(), deps: new Map(), idempotency: new Map(),
    rate: new Map(), taskNumbers: new Map(), nextUser: 1, nextProject: 1, nextTask: 1,
    nextComment: 1 };
}
function allowed(path: string): string[] | null {
  if (path === "/users" || path === "/sessions") return path === "/users" ? ["POST"] : ["POST", "DELETE"];
  if (path === "/me") return ["GET"];
  if (path === "/projects") return ["GET", "POST"];
  if (/^\/projects\/\d+$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/members$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/members\/\d+$/.test(path)) return ["DELETE"];
  if (/^\/projects\/\d+\/stats$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/tasks$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/tasks\/bulk$/.test(path)) return ["POST"];
  if (/^\/tasks\/[^/]+$/.test(path)) return ["GET", "PATCH", "DELETE"];
  if (/^\/tasks\/[^/]+\/transitions$/.test(path)) return ["POST"];
  if (/^\/tasks\/[^/]+\/history$/.test(path) || /^\/tasks\/[^/]+\/dependencies$/.test(path)) return ["GET", "POST"];
  if (/^\/tasks\/[^/]+\/dependencies\/[^/]+$/.test(path)) return ["DELETE"];
  if (/^\/tasks\/[^/]+\/comments$/.test(path)) return ["GET", "POST"];
  return null;
}
export function createApp(options: Options = {}): App {
  const ctx: Context = { state: state(), now: options.now ?? Date.now };
  return { async fetch(request: Request) {
    try {
      const url = new URL(request.url);
      const methods = allowed(url.pathname);
      if (!methods) fail(404, "not_found", "Route not found");
      if (!methods.includes(request.method)) {
        const result = errorResponse(new ApiError(405, "method_not_allowed", "Method not allowed"));
        result.headers.set("Allow", methods.join(", "));
        return result;
      }
      const token = bearer(request);
      let rateHeaders: Record<string, string> = {};
      if (token && ctx.state.sessions.has(token)) {
        const minute = Math.floor(ctx.now() / 60000);
        let entry = ctx.state.rate.get(token);
        if (!entry || entry.minute !== minute) entry = { minute, count: 0 };
        if (entry.count >= 100) {
          const retry = Math.ceil(((minute + 1) * 60000 - ctx.now()) / 1000);
          ctx.state.rate.set(token, entry);
          const res = errorResponse(new ApiError(429, "rate_limited", "Rate limit exceeded"));
          res.headers.set("Retry-After", String(retry));
          res.headers.set("X-RateLimit-Limit", "100");
          res.headers.set("X-RateLimit-Remaining", "0");
          return res;
        }
        entry.count++;
        ctx.state.rate.set(token, entry);
        rateHeaders = { "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": String(100 - entry.count) };
      }
      const result = await authRoute(ctx, request, url.pathname, request.method) ??
        await projectRoute(ctx, request, url, request.method) ??
        await taskRoute(ctx, request, url, request.method);
      if (!result) fail(404, "not_found", "Route not found");
      for (const [key, value] of Object.entries(rateHeaders)) result.headers.set(key, value);
      return result;
    } catch (error) { return errorResponse(error); }
  } };
}
