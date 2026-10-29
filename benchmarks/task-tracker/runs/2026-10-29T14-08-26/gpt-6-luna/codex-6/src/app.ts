import { state, stamp } from "./state";
import { answer, Fault, auth, bodyObj } from "./http";
import { account } from "./accounts";
import { tasksRoute } from "./tasks";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const s = state(),
    now = options.now || Date.now;
  return {
    async fetch(request) {
      const url = new URL(request.url),
        path = url.pathname,
        m = request.method.toUpperCase();
      try {
        const accepted = allowed(path);
        if (accepted && !accepted.includes(m))
          return answer(
            {
              error: {
                code: "method_not_allowed",
                message: "Method not allowed",
              },
            },
            405,
            { Allow: accepted.join(", ") },
          );
        const token = (request.headers.get("Authorization") || "").match(
          /^Bearer (.+)$/,
        )?.[1];
        if (token && s.sessions.has(token)) {
          const minute = Math.floor(now() / 60000),
            key = token,
            entry = s.rates.get(key) || { minute, count: 0 };
          if (entry.minute !== minute) {
            entry.minute = minute;
            entry.count = 0;
          }
          entry.count++;
          const headers = {
            "X-RateLimit-Limit": "100",
            "X-RateLimit-Remaining": String(Math.max(0, 100 - entry.count)),
          };
          s.rates.set(key, entry);
          if (entry.count > 100)
            return answer(
              { error: { code: "rate_limited", message: "Rate limited" } },
              429,
              {
                ...headers,
                "Retry-After": String(
                  Math.ceil((60000 - (now() % 60000)) / 1000),
                ),
              },
            );
          (request as any).__rate = headers;
        }
        if (["POST", "PATCH", "PUT"].includes(m)) {
          let value: any = {};
          try {
            value = await request.json();
          } catch {
            throw new Fault(400, "validation", "Invalid JSON");
          }
          (request as any).__body = bodyObj(value);
        }
        const a = account(s, request, m, path, now);
        if (a) return withRate(a, request);
        const t = tasksRoute(s, request, m, url, now);
        if (t) return withRate(t, request);
        return answer(
          { error: { code: "not_found", message: "Not found" } },
          404,
        );
      } catch (e) {
        if (e instanceof Fault) {
          const error: any = { code: e.code, message: e.message };
          if (e.field) error.field = e.field;
          return answer({ error }, e.status, rateHeaders(request));
        }
        return answer(
          { error: { code: "validation", message: "Invalid request" } },
          400,
          rateHeaders(request),
        );
      }
    },
  };
}
function rateHeaders(r: Request) {
  return (r as any).__rate || {};
}
function withRate(res: Response, r: Request) {
  const h = new Headers(res.headers);
  for (const [k, v] of Object.entries(rateHeaders(r))) h.set(k, v as string);
  return new Response(res.body, { status: res.status, headers: h });
}
function allowed(path: string): string[] | undefined {
  const p = path.split("/").filter(Boolean);
  if (path === "/users" || path === "/sessions")
    return path === "/users" ? ["POST"] : ["POST", "DELETE"];
  if (path === "/me") return ["GET"];
  if (path === "/projects") return ["GET", "POST"];
  if (p[0] === "projects") {
    if (p.length === 2) return ["GET"];
    if (p[2] === "members")
      return p.length === 3 ? ["GET", "POST"] : ["DELETE"];
    if (p[2] === "tasks") return p.length === 3 ? ["GET", "POST"] : ["POST"];
    if (p[2] === "stats") return ["GET"];
  }
  if (p[0] === "tasks") {
    if (p.length === 2) return ["GET", "PATCH", "DELETE"];
    if (["comments", "history", "dependencies"].includes(p[2]!))
      return p[2] === "comments"
        ? ["GET", "POST"]
        : p[2] === "history"
          ? ["GET"]
          : p.length === 3
            ? ["GET", "POST"]
            : ["DELETE"];
    if (p[2] === "transitions") return ["POST"];
  }
  return undefined;
}
