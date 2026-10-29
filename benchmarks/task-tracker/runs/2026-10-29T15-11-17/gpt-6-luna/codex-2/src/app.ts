import { err, key, ok, type State } from "./model";
import { usersRoute } from "./routes-users";
import { dataRoute } from "./routes-data";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
const methods: Record<string, string> = {
  "/users": "POST",
  "/sessions": "POST, DELETE",
  "/me": "GET",
};
export function createApp(options: Options = {}): App {
  const s: State = {
    users: [],
    projects: [],
    tasks: [],
    sessions: new Map(),
    memberships: new Map(),
    comments: new Map(),
    idem: new Map(),
    rates: new Map(),
    now: options.now ?? Date.now,
  };
  return {
    async fetch(request: Request) {
      const url = new URL(request.url),
        path = url.pathname,
        method = request.method;
      const route = path.match(/^\/(users|sessions|me)$/) ? path : null;
      const patterns = [
        /^\/projects$/,
        /^\/users$/,
        /^\/sessions$/,
        /^\/me$/,
        /^\/projects\/[^/]+$/,
        /^\/projects\/[^/]+\/(members|tasks|tasks\/bulk|stats)$/,
        /^\/projects\/[^/]+\/members\/[^/]+$/,
        /^\/tasks\/[^/]+(?:\/.*)?$/,
      ];
      let allowed = route ? methods[route] : undefined;
      if (path === "/projects") allowed = "GET, POST";
      if (!allowed && patterns.some((r) => r.test(path)))
        allowed = method === "GET" ? "GET" : "GET, POST, PATCH, DELETE";
      if (!allowed) return err("not_found", "not found", 404);
      if (!allowed.split(", ").includes(method))
        return new Response(
          JSON.stringify({ error: { code: "method_not_allowed", message: "method not allowed" } }),
          { status: 405, headers: { "Content-Type": "application/json", Allow: allowed } },
        );
      const auth = request.headers.get("Authorization") ?? "",
        token = auth.startsWith("Bearer ") ? auth.slice(7) : "";
      const who = s.sessions.get(token);
      let rateHeaders: Record<string, string> = {};
      if (token && who) {
        let r = s.rates.get(token);
        if (!r || s.now() - r.start >= 60000) {
          r = { start: Math.floor(s.now() / 60000) * 60000, count: 0 };
          s.rates.set(token, r);
        }
        if (r.count >= 100) {
          const retry = Math.ceil((60000 - (s.now() - r.start)) / 1000);
          return Response.json(
            { error: { code: "rate_limited", message: "rate limited" } },
            {
              status: 429,
              headers: {
                "Retry-After": String(retry),
                "X-RateLimit-Limit": "100",
                "X-RateLimit-Remaining": "0",
              },
            },
          );
        }
        r.count++;
        rateHeaders = {
          "X-RateLimit-Limit": "100",
          "X-RateLimit-Remaining": String(100 - r.count),
        };
      }
      let body: any = {};
      if (request.method !== "GET" && request.method !== "DELETE") {
        try {
          const b = await request.json();
          if (!b || typeof b !== "object" || Array.isArray(b))
            return err("validation", "body must be an object", 400);
          body = b;
        } catch {
          return err("validation", "body must be valid JSON", 400);
        }
      }
      if (["/me", "/sessions"].includes(path) && method !== "POST" && !who)
        return err("unauthorized", "unauthorized", 401);
      let response = await usersRoute(s, method, path, body, who);
      if (!response)
        response = await dataRoute(s, method, path, url, body, who, token, request.headers);
      if (!response) response = err("not_found", "not found", 404);
      for (const [k, v] of Object.entries(rateHeaders)) response.headers.set(k, v);
      return response;
    },
  };
}
