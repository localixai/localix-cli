import { err, type Ctx } from "./http";
import { projects } from "./projects";
import { taskRoutes } from "./tasks";
import { users } from "./users";
import type { State } from "./model";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const s: State = { users: [], projects: [], tasks: [], tokens: new Map(), members: new Map(),
    comments: new Map(), history: new Map(), deps: new Map(), rates: new Map(), idem: new Map(),
    id: 0, now: options.now || Date.now };
  return { async fetch(req) {
    const url = new URL(req.url), path = url.pathname;
    const methods: Record<string, string[]> = {
      "/users": ["POST"], "/sessions": ["POST", "DELETE"], "/me": ["GET"],
      "/projects": ["GET", "POST"],
    };
    let allowed = methods[path];
    if (/^\/projects\/\d+$/.test(path)) allowed = ["GET"];
    if (/^\/projects\/\d+\/members$/.test(path)) allowed = ["GET", "POST"];
    if (/^\/projects\/\d+\/members\/\d+$/.test(path)) allowed = ["DELETE"];
    if (/^\/projects\/\d+\/(stats|tasks)$/.test(path)) allowed = ["GET", "POST"];
    if (/^\/projects\/\d+\/tasks\/bulk$/.test(path)) allowed = ["POST"];
    if (/^\/tasks\/[^/]+$/.test(path)) allowed = ["GET", "PATCH", "DELETE"];
    if (/^\/tasks\/[^/]+\/(comments|dependencies)$/.test(path)) allowed = ["GET", "POST"];
    if (/^\/tasks\/[^/]+\/(history)$/.test(path)) allowed = ["GET"];
    if (/^\/tasks\/[^/]+\/(transitions)$/.test(path)) allowed = ["POST"];
    if (/^\/tasks\/[^/]+\/dependencies\/[^/]+$/.test(path)) allowed = ["DELETE"];
    if (!allowed) return err("not_found", "Not found", undefined, 404);
    if (!allowed.includes(req.method)) {
      const r = err("method_not_allowed", "Method not allowed", undefined, 405);
      r.headers.set("Allow", allowed.join(", ")); return r;
    }
    const token = req.headers.get("Authorization")?.match(/^Bearer (.+)$/)?.[1];
    const user = token ? s.tokens.get(token) : null;
    const authNeeded = path !== "/users" && path !== "/sessions";
    if (authNeeded && user == null) return err("unauthorized", "Unauthorized", undefined, 401);
    const now = new Date(s.now()).toISOString();
    const c: Ctx = { s, req, url, user: user ?? null, now };
    if (user != null) {
      const minute = Math.floor(s.now() / 60000), rate = s.rates.get(token!) || { minute, count: 0 };
      if (rate.minute !== minute) { rate.minute = minute; rate.count = 0; }
      rate.count++; s.rates.set(token!, rate);
      const headers = { "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": String(Math.max(0, 100 - rate.count)) };
      if (rate.count > 100) {
        const r = err("rate_limited", "Rate limited", undefined, 429);
        r.headers.set("Retry-After", String(Math.ceil(((minute + 1) * 60000 - s.now()) / 1000)));
        Object.entries(headers).forEach(([k, v]) => r.headers.set(k, v)); return r;
      }
      const result = await users(c, path) || await projects(c, path) || await taskRoutes(c, path);
      if (!result) return err("not_found", "Not found", undefined, 404);
      Object.entries(headers).forEach(([k, v]) => result.headers.set(k, v));
      return result;
    }
    return await users(c, path) || err("not_found", "Not found", undefined, 404);
  } };
}
