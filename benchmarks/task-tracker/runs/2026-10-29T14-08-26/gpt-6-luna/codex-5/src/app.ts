import { store } from "./model";
import { HttpError, errorResponse, json, obj, fail } from "./http";
import { account } from "./accounts";
import { taskRoute } from "./tasks";
import { stats } from "./stats";
import type { Ctx } from "./context";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const s = store(),
    now = options.now ?? Date.now;
  return {
    async fetch(req: Request) {
      const url = new URL(req.url),
        path = url.pathname,
        method = req.method.toUpperCase();
      const allowed: Record<string, string> = {
        "/users": "POST",
        "/sessions": "POST, DELETE",
        "/me": "GET",
      };
      if (path === "/projects") allowed[path] = "GET, POST";
      if (method === "OPTIONS") return new Response(null, { status: 204 });
      const methods = routeMethods(path);
      if (methods && !methods.includes(method))
        return Response.json(
          {
            error: {
              code: "method_not_allowed",
              message: "Method not allowed",
            },
          },
          { status: 405, headers: { Allow: methods.join(", ") } },
        );
      if (!methods)
        return errorResponse(new HttpError(404, "not_found", "Not found"));
      let body: Record<string, any> = {};
      if (!["GET", "DELETE", "HEAD"].includes(method)) {
        try {
          body = obj(await req.json());
        } catch (e) {
          if (e instanceof HttpError) return errorResponse(e);
          return errorResponse(
            new HttpError(400, "validation", "Invalid JSON"),
          );
        }
      }
      const token = req.headers
        .get("Authorization")
        ?.match(/^Bearer (.+)$/)?.[1];
      const uid = token ? s.sessions.get(token) : undefined;
      const user = uid ? s.users.find((u) => u.id === uid) : undefined;
      if (token && user) {
        const minute = Math.floor(now() / 60000),
          rec = s.counts.get(token);
        const count = rec?.minute === minute ? rec.count : 0;
        const headers = {
          "X-RateLimit-Limit": "100",
          "X-RateLimit-Remaining": String(Math.max(0, 99 - count)),
        };
        if (count >= 100)
          return Response.json(
            { error: { code: "rate_limited", message: "Rate limited" } },
            {
              status: 429,
              headers: {
                ...headers,
                "Retry-After": String(
                  Math.ceil((60000 - (now() % 60000)) / 1000),
                ),
              },
            },
          );
        s.counts.set(token, { minute, count: count + 1 });
      }
      if (
        !["/users", "/sessions"].includes(path) &&
        !(path === "/sessions" && method === "POST")
      ) {
        if (!user)
          return errorResponse(
            new HttpError(401, "unauthorized", "Unauthorized"),
          );
      }
      try {
        if ((path === "/users" && method === "POST") || path === "/sessions") {
          const guest = { id: 0, email: "", name: "", password: "" };
          const c = {
            s,
            now,
            user: user ?? guest,
            body,
            req,
            params: [],
            url,
          } as Ctx;
          const response = account(c, path, method);
          if (response) return response;
        }
        if (!user)
          return errorResponse(
            new HttpError(401, "unauthorized", "Unauthorized"),
          );
        const c: Ctx = { s, now, user, body, req, params: [], url };
        const response =
          account(c, path, method) ??
          taskRoute(c, path, method) ??
          stats(c, path, method);
        if (response && token && s.counts.has(token)) {
          const count = s.counts.get(token)!.count;
          response.headers.set("X-RateLimit-Limit", "100");
          response.headers.set(
            "X-RateLimit-Remaining",
            String(Math.max(0, 100 - count)),
          );
        }
        return (
          response ??
          errorResponse(new HttpError(404, "not_found", "Not found"))
        );
      } catch (e) {
        return errorResponse(e);
      }
    },
  };
}
function routeMethods(path: string): string[] | undefined {
  if (path === "/users") return ["POST"];
  if (path === "/sessions") return ["POST", "DELETE"];
  if (path === "/me") return ["GET"];
  if (path === "/projects") return ["GET", "POST"];
  if (/^\/projects\/\d+$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/members$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/members\/\d+$/.test(path)) return ["DELETE"];
  if (/^\/projects\/\d+\/(tasks|stats)(\/bulk)?$/.test(path))
    return path.endsWith("/stats") ? ["GET"] : ["GET", "POST"];
  if (/^\/tasks\/[^/]+$/.test(path)) return ["GET", "PATCH", "DELETE"];
  if (/^\/tasks\/[^/]+\/(transitions|dependencies|comments)$/.test(path))
    return path.endsWith("/transitions") ? ["POST"] : ["GET", "POST"];
  if (/^\/tasks\/[^/]+\/(history)$/.test(path)) return ["GET"];
  if (/^\/tasks\/[^/]+\/comments$/.test(path)) return ["GET", "POST"];
  if (/^\/tasks\/[^/]+\/dependencies\/[^/]+$/.test(path)) return ["DELETE"];
  return undefined;
}
