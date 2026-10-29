import { fresh, type State } from "./model";
import { ApiError, auth, body, date, json, type Ctx } from "./util";
import { users } from "./users";
import { projects } from "./projects";
import { tasks } from "./tasks";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const state: State = fresh(); const now = options.now || Date.now;
  return { async fetch(request) {
    const ctx: Ctx = { state, now, request };
    try {
      const method = request.method, path = new URL(request.url).pathname;
      const known: Record<string, string> = {
        "/users": "POST", "/sessions": "POST, DELETE", "/me": "GET",
        "/projects": "GET, POST",
      };
      if (path in known && !known[path]!.split(", ").includes(method)) {
        return json({ error: { code: "method_not_allowed", message: "Method not allowed" } }, 405,
          { Allow: known[path]! });
      }
      const limited = auth(ctx, false);
      let limitHeaders: Record<string, string> = {};
      if (limited.token) {
        const minute = Math.floor(now() / 60000);
        let entry = state.limits.get(limited.token);
        if (!entry || entry.minute !== minute) entry = { minute, count: 0 };
        entry.count++; state.limits.set(limited.token, entry);
        const remaining = Math.max(0, 100 - entry.count);
        limitHeaders = { "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": String(remaining) };
        if (entry.count > 100) {
          const retry = Math.ceil(((minute + 1) * 60000 - now()) / 1000);
          return json({ error: { code: "rate_limited", message: "Rate limit exceeded" } }, 429,
            { ...limitHeaders, "Retry-After": String(retry) });
        }
      }
      const r = await users(ctx, method, path) || await projects(ctx, method, path) ||
        await tasks(ctx, method, path);
      if (r) { for (const [k, v] of Object.entries(limitHeaders)) r.headers.set(k, v); return r; }
      return json({ error: { code: "not_found", message: "Not found" } }, 404, limitHeaders);
    } catch (error) {
      if (error instanceof ApiError) {
        const detail: Record<string, unknown> = { code: error.code, message: error.message };
        if (error.field) detail.field = error.field;
        return json({ error: detail }, error.status);
      }
      return json({ error: { code: "validation", message: "Invalid request" } }, 400);
    }
  } };
}
