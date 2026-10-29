import type { State } from "./model";
import { account } from "./accounts";
import { projects } from "./projects";
import { taskCollection } from "./task-create";
import { taskDetail } from "./task-detail";
import { ApiError, auth, errorResponse, respond } from "./http";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
function methodAllowed(path: string): string | null {
  if (path === "/users" || path === "/sessions")
    return path === "/users" ? "POST" : "POST, DELETE";
  if (path === "/me") return "GET";
  if (path === "/projects") return "GET, POST";
  if (/^\/projects\/\d+$/.test(path)) return "GET";
  if (/^\/projects\/\d+\/members$/.test(path)) return "GET, POST";
  if (/^\/projects\/\d+\/members\/\d+$/.test(path)) return "DELETE";
  if (/^\/projects\/\d+\/stats$/.test(path)) return "GET";
  if (/^\/projects\/\d+\/tasks$/.test(path)) return "GET, POST";
  if (/^\/projects\/\d+\/tasks\/bulk$/.test(path)) return "POST";
  if (/^\/tasks\/[^/]+$/.test(path)) return "GET, PATCH, DELETE";
  if (/^\/tasks\/[^/]+\/transitions$/.test(path)) return "POST";
  if (/^\/tasks\/[^/]+\/(history|comments|dependencies)$/.test(path))
    return "GET, POST";
  if (/^\/tasks\/[^/]+\/dependencies\/[^/]+$/.test(path)) return "DELETE";
  return null;
}
export function createApp(options: Options = {}): App {
  const s: State = {
    users: [],
    projects: [],
    tasks: [],
    members: new Map(),
    counters: new Map(),
    sessions: new Map(),
    idem: new Map(),
    rates: new Map(),
    now: options.now || Date.now,
  };
  return {
    async fetch(req) {
      try {
        const url = new URL(req.url),
          path = url.pathname,
          allow = methodAllowed(path);
        if (!allow)
          return respond(
            { error: { code: "not_found", message: "not found" } },
            404,
          );
        if (!allow.split(", ").includes(req.method))
          return respond(
            {
              error: {
                code: "method_not_allowed",
                message: "method not allowed",
              },
            },
            405,
            { Allow: allow },
          );
        const token = /^Bearer (.+)$/.exec(
          req.headers.get("Authorization") || "",
        )?.[1];
        if (token && path !== "/sessions" && path !== "/users") {
          const minute = Math.floor(s.now() / 60000),
            rate = s.rates.get(token);
          const count = rate?.minute === minute ? rate.count : 0;
          if (count >= 100) {
            const retry = Math.ceil(((minute + 1) * 60000 - s.now()) / 1000);
            return respond(
              {
                error: { code: "rate_limited", message: "rate limit exceeded" },
              },
              429,
              {
                "Retry-After": String(retry),
                "X-RateLimit-Limit": "100",
                "X-RateLimit-Remaining": "0",
              },
            );
          }
          s.rates.set(token, { minute, count: count + 1 });
        }
        const res = await account(req, path, s);
        if (res) return addRate(res, token, s);
        const p = await projects(req, url, s);
        if (p) return addRate(p, token, s);
        const collection = /^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/.exec(path);
        if (collection) {
          const uid = auth(req, s);
          (req as any).uid = uid;
          const result = await taskCollection(
            req,
            url,
            s,
            Number(collection[1]),
            !!collection[2],
          );
          return addRate(result, token, s);
        }
        const detail = await taskDetail(req, path, s, auth(req, s));
        if (detail) return addRate(detail, token, s);
        return respond(
          { error: { code: "not_found", message: "not found" } },
          404,
        );
      } catch (e) {
        return errorResponse(e);
      }
    },
  };
}
function addRate(
  response: Response,
  token: string | undefined,
  s: State,
): Response {
  if (!token) return response;
  const r = s.rates.get(token);
  const h = new Headers(response.headers);
  h.set("X-RateLimit-Limit", "100");
  h.set("X-RateLimit-Remaining", String(Math.max(0, 100 - (r?.count || 0))));
  return new Response(response.body, { status: response.status, headers: h });
}
