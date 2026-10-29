import { initial } from "./model";
import { people, projects } from "./people";
import { bad, fail, gone } from "./util";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const s = initial();
  const now = options.now ?? Date.now;
  const counts = new Map<string, { start: number; count: number }>();
  return { async fetch(request) {
    const url = new URL(request.url);
    const method = request.method;
    const path = url.pathname;
    const allowed: Record<string, string> = {
      "/users": "POST", "/sessions": "POST, DELETE", "/me": "GET",
      "/projects": "GET, POST",
    };
    if (allowed[path] && !allowed[path].split(", ").includes(method)) {
      return fail("method_not_allowed", "Method not allowed", 405);
    }
    const token = request.headers.get("Authorization")?.match(/^Bearer (.+)$/)?.[1];
    const user = token ? s.sessions.get(token) ?? null : null;
    let limitHeaders: Record<string, string> = {};
    if (token && user) {
      let slot = counts.get(token);
      if (!slot || now() >= slot.start + 60000) {
        slot = { start: Math.floor(now() / 60000) * 60000, count: 0 };
        counts.set(token, slot);
      }
      if (slot.count >= 100) return Response.json({ error: { code: "rate_limited",
        message: "Too many requests" } }, { status: 429, headers: {
        "Retry-After": String(Math.ceil((slot.start + 60000 - now()) / 1000)),
        "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "0",
      } });
      slot.count++;
      limitHeaders = { "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": String(100 - slot.count) };
    }
    let body: any = {};
    if (["POST", "PATCH", "PUT"].includes(method)) {
      try {
        body = await request.json();
        if (!body || typeof body !== "object" || Array.isArray(body)) return bad("");
      } catch { return fail("validation", "Invalid JSON", 400); }
    }
    let response: Response | null = null;
    if (path === "/users" || path === "/sessions") {
      response = people(s, method, path, body, user);
      if (method === "DELETE" && path === "/sessions" && token) s.sessions.delete(token);
    }
    if (path === "/me" && method === "GET") response = people(s, method, path, body, user);
    if (path.startsWith("/projects") && method !== "POST" && method !== "GET") {
      response = null;
    }
    if (path === "/projects" && method === "GET" || path === "/projects" && method === "POST") {
      if (user === null) response = fail("unauthorized", "Unauthorized", 401);
      else response = projects(s, method, path, body, user, new Date(now()).toISOString());
    }
    if (response) {
      const headers = new Headers(response.headers);
      for (const [k, v] of Object.entries(limitHeaders)) headers.set(k, v);
      return new Response(response.body, { status: response.status, headers });
    }
    return gone();
  } };
}
