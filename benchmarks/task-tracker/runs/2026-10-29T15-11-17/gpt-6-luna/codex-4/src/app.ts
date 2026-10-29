import type { App } from "./types";
import type { State } from "./model";
import { identity } from "./identity";
import { projects } from "./projects";
import { tasks } from "./tasks";
import { stats } from "./stats";
import { Fault, fail } from "./util";

export type Options = { now?: () => number };
export type { App } from "./types";

export function createApp(options: Options = {}): App {
  const s: State = {
    users: [],
    projects: [],
    members: new Map(),
    tokens: new Map(),
    tasks: [],
    comments: new Map(),
    history: new Map(),
    deps: new Map(),
    idems: new Map(),
    counts: new Map(),
  };
  const now = options.now || Date.now;
  return {
    async fetch(request: Request) {
      const url = new URL(request.url),
        method = request.method,
        path = url.pathname;
      const responseHeaders = new Headers();
      const allow = allowed(path);
      if (!allow) return json({ error: { code: "not_found", message: "route not found" } }, 404, responseHeaders);
      if (!allow.includes(method)) {
        responseHeaders.set("Allow", allow.join(", "));
        return json(
          {
            error: {
              code: "method_not_allowed",
              message: "method not allowed",
            },
          },
          405,
          responseHeaders,
        );
      }
      const token = /^Bearer (.+)$/.exec(request.headers.get("Authorization") || "")?.[1];
      const uid = token ? s.tokens.get(token) : undefined;
      if (token && uid) {
        const minute = Math.floor(now() / 60000) * 60000;
        const count = s.counts.get(token);
        const state = !count || count.start !== minute ? { start: minute, count: 0 } : count;
        if (!count || count.start !== minute) s.counts.set(token, state);
        state.count++;
        responseHeaders.set("X-RateLimit-Limit", "100");
        responseHeaders.set("X-RateLimit-Remaining", String(Math.max(0, 100 - state.count)));
        if (state.count > 100) {
          responseHeaders.set("Retry-After", String(Math.ceil((minute + 60000 - now()) / 1000)));
          return json({ error: { code: "rate_limited", message: "rate limit exceeded" } }, 429, responseHeaders);
        }
      }
      const publicRoute = method === "POST" && ["/users", "/sessions"].includes(path);
      const signout = path === "/sessions" && method === "DELETE";
      if (!publicRoute && !uid)
        return json(
          {
            error: { code: "unauthorized", message: "authentication required" },
          },
          401,
          responseHeaders,
        );
      try {
        const body = await parse(request);
        let result: any = await identity(s, method, path, body, uid);
        if (!result) result = projects(s, method, path, body, uid, now());
        if (!result && uid) result = tasks(s, method, path, url, body, uid, now(), request.headers);
        if (!result && method === "GET" && /^\/projects\/\d+\/stats$/.test(path)) result = stats(s, path, uid!);
        if (!result) fail(404, "not_found", "route not found");
        if ("etag" in result && result.etag) responseHeaders.set("ETag", `"${result.etag}"`);
        if ("replay" in result && result.replay) responseHeaders.set("Idempotent-Replayed", "true");
        return json(result.body, result.status, responseHeaders);
      } catch (error) {
        if (error instanceof Fault) {
          const detail: Record<string, string> = {
            code: error.code,
            message: error.message,
          };
          if (error.field) detail.field = error.field;
          return json({ error: detail }, error.status, responseHeaders);
        }
        return json({ error: { code: "internal", message: "internal error" } }, 500, responseHeaders);
      }
    },
  };
}
const parse = async (r: Request) => {
  if (!r.headers.get("content-type")?.includes("application/json")) return {};
  try {
    const x = await r.json();
    if (!x || typeof x !== "object" || Array.isArray(x)) {
      fail(400, "validation", "body must be a JSON object");
    }
    return x as Record<string, any>;
  } catch (error) {
    if (error instanceof Fault) throw error;
    fail(400, "validation", "body must be valid JSON");
  }
};
function allowed(path: string): string[] | undefined {
  if (path === "/users") return ["POST"];
  if (path === "/sessions") return ["POST", "DELETE"];
  if (path === "/me") return ["GET"];
  if (path === "/projects") return ["GET", "POST"];
  if (/^\/projects\/\d+$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/members$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/members\/\d+$/.test(path)) return ["DELETE"];
  if (/^\/projects\/\d+\/tasks$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/stats$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/tasks\/bulk$/.test(path)) return ["POST"];
  if (/^\/tasks\/[^/]+$/.test(path)) return ["GET", "PATCH", "DELETE"];
  if (/^\/tasks\/[^/]+\/history$/.test(path)) return ["GET"];
  if (/^\/tasks\/[^/]+\/dependencies$/.test(path)) return ["GET", "POST"];
  if (/^\/tasks\/[^/]+\/(comments)$/.test(path)) return ["GET", "POST"];
  if (/^\/tasks\/[^/]+\/transitions$/.test(path)) return ["POST"];
  if (/^\/tasks\/[^/]+\/dependencies\/[^/]+$/.test(path)) return ["DELETE"];
  return undefined;
}
const json = (body: unknown, status: number, headers: Headers) =>
  body === null ? new Response(null, { status, headers }) : Response.json(body, { status, headers });
