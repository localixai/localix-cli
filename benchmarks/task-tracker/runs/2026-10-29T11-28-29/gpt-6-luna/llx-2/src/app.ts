import { State, bodyObject, error } from "./model";
import { userRoute } from "./users";
import { projectRoute } from "./projects";
import { taskRoute } from "./tasks";
import { dependencyRoute } from "./dependencies";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const s: State = { now: options.now ?? Date.now, users: [], projects: [], tasks: [],
    members: new Map(), tokens: new Map(), comments: new Map(), history: new Map(), deps: new Map(),
    limits: new Map(), idem: new Map(), taskNumbers: new Map(),
    ids: { user: 0, project: 0, task: 0, comment: 0, token: 0 } };
  return { async fetch(request) {
    const url = new URL(request.url), method = request.method, path = url.pathname;
    const allowed = methods(path);
    if (!allowed) return error("not_found", "Not found", 404);
    if (!allowed.includes(method)) return error("method_not_allowed", "Method not allowed", 405,
      undefined, new Headers({ Allow: allowed.join(", ") }));
    let data: Record<string, any> | null = null;
    if (["POST", "PATCH", "PUT"].includes(method)) {
      try { data = bodyObject(await request.json()); } catch { return error("validation", "Invalid JSON", 400); }
      if (!data) return error("validation", "Expected an object", 400);
    }
    const token = request.headers.get("Authorization")?.match(/^Bearer (.+)$/)?.[1];
    const uid = token ? s.tokens.get(token) : undefined;
    let rateHeaders: Headers | undefined;
    if (!["/users", "/sessions"].includes(path) && uid === undefined)
      return error("unauthorized", "Unauthorized", 401);
    if (uid !== undefined) {
      const minute = Math.floor(s.now() / 60000);
      let limit = s.limits.get(token!);
      if (!limit || limit.start !== minute) limit = { start: minute, count: 0 };
      limit.count++; s.limits.set(token!, limit);
      const h = new Headers({ "X-RateLimit-Limit": "100",
        "X-RateLimit-Remaining": String(Math.max(0, 100 - limit.count)) });
      rateHeaders = h;
      if (limit.count > 100) {
        h.set("Retry-After", String(Math.ceil(((minute + 1) * 60000 - s.now()) / 1000)));
        return error("rate_limited", "Rate limit exceeded", 429, undefined, h);
      }
    }
    const headers = request.headers;
    const response = userRoute(s, method, path, uid, token, data) ??
      projectRoute(s, method, path, uid!, data) ??
      dependencyRoute(s, method, path, uid!, data) ??
      taskRoute(s, method, path, uid!, data, url, headers) ??
      error("not_found", "Not found", 404);
    if (rateHeaders) rateHeaders.forEach((value, key) => response.headers.set(key, value));
    return response;
  } };
}
function methods(path: string): string[] | undefined {
  if (path === "/users" || path === "/sessions") return path === "/users" ? ["POST"] : ["POST", "DELETE"];
  if (path === "/me") return ["GET"];
  if (path === "/projects") return ["GET", "POST"];
  if (/^\/projects\/\d+$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/members$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/members\/\d+$/.test(path)) return ["DELETE"];
  if (/^\/projects\/\d+\/(tasks|stats)$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/tasks\/bulk$/.test(path)) return ["POST"];
  if (/^\/tasks\/[A-Z]+-\d+$/.test(path)) return ["GET", "PATCH", "DELETE"];
  if (/^\/tasks\/[A-Z]+-\d+\/(comments|history|dependencies)$/.test(path)) return ["GET", "POST"];
  if (/^\/tasks\/[A-Z]+-\d+\/transitions$/.test(path)) return ["POST"];
  if (/^\/tasks\/[A-Z]+-\d+\/dependencies\/[A-Z]+-\d+$/.test(path)) return ["DELETE"];
}
