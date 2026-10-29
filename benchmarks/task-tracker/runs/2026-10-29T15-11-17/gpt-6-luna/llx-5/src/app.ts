import type { Store } from "./model";
import { auth, errorResponse, fail, ok, Fault } from "./http";
import { users } from "./users";
import { projects } from "./projects";
import { tasks } from "./tasks";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const s: Store = {
    users: [], projects: [], members: new Map(), tasks: [], sessions: new Map(),
    history: new Map(), comments: new Map(), limits: new Map(), idem: new Map(),
    now: options.now ?? Date.now, uid: 0, pid: 0, tid: 0,
  };
  return { fetch: (request) => handle(request, s) };
}
async function handle(req: Request, s: Store): Promise<Response> {
  try {
    const url = new URL(req.url), path = url.pathname, method = req.method;
    const allowed = methods(path);
    if (!allowed) return errorResponse(new Fault(404, "not_found", "not found"));
    if (!allowed.includes(method)) {
      const response = errorResponse(new Fault(405, "method_not_allowed", "method not allowed"));
      const headers = { ...Object.fromEntries(response.headers), Allow: allowed.join(", ") };
      return new Response(response.body, { status: 405, headers });
    }
    if (method === "GET" && path === "/me") {
      const u = auth(req, s); rate(s, u.token);
      const limited = rateCheck(s, u.token);
      if (limited) return limited;
      const user = s.users.find((x) => x.id === u.id)!;
      return json({ id: user.id, email: user.email, name: user.name }, 200, rateHeaders(s, u.token));
    }
    if (path === "/sessions" && method === "DELETE") {
      const u = auth(req, s); rate(s, u.token); s.sessions.delete(u.token);
      return json(null, 204, rateHeaders(s, u.token));
    }
    if (method !== "POST" && path !== "/users" && path !== "/sessions") {
      const u = auth(req, s); rate(s, u.token);
      const limited = rateCheck(s, u.token);
      if (limited) return limited;
    }
    const result = await users(req, path, s) ?? await projects(req, url, s) ?? await tasks(req, url, s);
    if (result) return result;
    return errorResponse(new Error("not found"));
  } catch (e) {
    return errorResponse(e);
  }
}
function methods(path: string): string[] | null {
  if (path === "/users" || path === "/sessions") return path === "/users" ? ["POST"] : ["POST", "DELETE"];
  if (path === "/me") return ["GET"];
  if (path === "/projects") return ["GET", "POST"];
  if (/^\/projects\/\d+$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/(members|stats)$/.test(path)) return path.endsWith("stats") ? ["GET"] : ["GET", "POST"];
  if (/^\/projects\/\d+\/members\/\d+$/.test(path)) return ["DELETE"];
  if (/^\/projects\/\d+\/tasks$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/tasks\/bulk$/.test(path)) return ["POST"];
  if (/^\/tasks\/[A-Z]+-\d+$/.test(path)) return ["GET", "PATCH", "DELETE"];
  if (/^\/tasks\/[A-Z]+-\d+\/(history|dependencies)$/.test(path)) return ["GET", "POST"];
  if (/^\/tasks\/[A-Z]+-\d+\/dependencies\/[A-Z]+-\d+$/.test(path)) return ["DELETE"];
  if (/^\/tasks\/[A-Z]+-\d+\/(comments|transitions)$/.test(path)) return ["GET", "POST"];
  return null;
}
function json(body: any, status: number, headers: Record<string, string>) {
  return ok(body, status, headers);
}
function rate(s: Store, token: string) {
  const minute = Math.floor(s.now() / 60_000) * 60_000, value = s.limits.get(token);
  if (!value || value.start !== minute) s.limits.set(token, { start: minute, count: 0 });
}
function rateCheck(s: Store, token: string): Response | null {
  const value = s.limits.get(token)!;
  if (value.count >= 100) {
    const retry = Math.ceil((value.start + 60_000 - s.now()) / 1000);
    return json({ error: { code: "rate_limited", message: "rate limited" } }, 429, {
      "Retry-After": String(retry), "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "0",
    });
  }
  value.count++;
  return null;
}
function rateHeaders(s: Store, token: string): Record<string, string> {
  return { "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": String(100 - (s.limits.get(token)?.count ?? 0)) };
}
