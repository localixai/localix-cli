import type { Ctx, Store } from "./types";
import { people } from "./people";
import { projects } from "./projects";
import { taskList } from "./task-list";
import { readTaskRoute } from "./task-read";
import { fail } from "./util";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

const methods: Record<string, string> = {
  "/users": "POST", "/sessions": "POST, DELETE", "/me": "GET", "/projects": "GET, POST",
};
export function createApp(options: Options = {}): App {
  const store: Store = { users: [], projects: [], tasks: [], tokens: new Map(),
    comments: new Map(), history: new Map(), now: options.now ?? Date.now, nextToken: 1,
    limits: new Map(), idempotency: new Map() };
  return { async fetch(request) {
    const url = new URL(request.url);
    const allow = methods[url.pathname];
    if (allow && !allow.split(", ").includes(request.method)) {
      const response = fail("method_not_allowed", "Method not allowed", 405);
      response.headers.set("Allow", allow);
      return response;
    }
    let user = null;
    const auth = request.headers.get("Authorization") ?? "";
    const token = auth.startsWith("Bearer ") ? auth.slice(7) : "";
    if (token && store.tokens.has(token)) {
      const state = store.limits.get(token);
      const minute = Math.floor(store.now() / 60_000);
      if (!state || state.minute !== minute) store.limits.set(token, { minute, count: 0 });
      const current = store.limits.get(token)!;
      current.count++;
      const headers = { "X-RateLimit-Limit": "100",
        "X-RateLimit-Remaining": String(Math.max(0, 100 - current.count)) };
      if (current.count > 100) return new Response(JSON.stringify({
        error: { code: "rate_limited", message: "Rate limit exceeded" },
      }), { status: 429, headers: { ...headers, "Content-Type": "application/json",
        "Retry-After": String(Math.ceil(((current.minute + 1) * 60_000 - store.now()) / 1000)) } });
      user = store.users.find(u => u.id === store.tokens.get(token)) ?? null;
      const ctx = { store, user, request, url, params: [] } as Ctx;
      const response = await dispatch(ctx);
      response.headers.set("X-RateLimit-Limit", "100");
      response.headers.set("X-RateLimit-Remaining", String(Math.max(0, 100 - current.count)));
      return response;
    }
    const needsAuth = url.pathname === "/me" || url.pathname === "/projects" ||
      url.pathname.startsWith("/projects/") ||
      url.pathname.startsWith("/tasks/");
    if (needsAuth) return fail("unauthorized", "Unauthorized", 401);
    return await dispatch({ store, user: null, request, url, params: [] });
  } };
}

async function dispatch(ctx: Ctx): Promise<Response> {
  const results = await people(ctx);
  if (results) return results;
  if (ctx.url.pathname === "/me") return fail("unauthorized", "Unauthorized", 401);
  const p = await projects(ctx);
  if (p) return p;
  const list = await taskList(ctx);
  if (list) return list;
  const task = await readTaskRoute(ctx);
  if (task) return task;
  return fail("not_found", "Not found", 404);
}
