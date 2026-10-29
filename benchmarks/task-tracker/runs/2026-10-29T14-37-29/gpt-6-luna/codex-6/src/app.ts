import type { App, Options } from "./app-types";
import type { Ctx, Result, State } from "./types";
import { fail } from "./util";
import { userRoute } from "./users";
import { projectRoute } from "./projects";
import { taskRoute } from "./tasks";
import { listRoute } from "./listing";
import { createTask } from "./tasks";
export type { App, Options } from "./app-types";
export function createApp(options: Options = {}): App {
  const s: State = {
    users: [],
    sessions: new Map(),
    projects: [],
    members: new Map(),
    tasks: [],
    histories: new Map(),
    comments: new Map(),
    deps: new Map(),
    idem: new Map(),
    limits: new Map(),
    now: options.now || Date.now,
    ids: {},
  };
  return {
    fetch: async (req) => {
      const url = new URL(req.url),
        path = url.pathname,
        m = req.method;
      let user = null;
      const token = req.headers
        .get("Authorization")
        ?.match(/^Bearer (.+)$/)?.[1];
      if (token) {
        const id = s.sessions.get(token);
        user = s.users.find((u) => u.id === id) || null;
      }
      const methods = allowed(path);
      if (methods && !methods.includes(m))
        return response({
          ...fail(405, "method_not_allowed"),
          headers: { Allow: methods.join(", ") },
        });
      let body: any = {};
      if (["POST", "PATCH", "PUT"].includes(m)) {
        try {
          body = JSON.parse(await req.text());
        } catch {
          return response(fail(400, "validation", "invalid JSON"));
        }
      }
      const ctx: Ctx = { state: s, req, user, body, url };
      if (
        ["POST", "PATCH", "PUT"].includes(m) &&
        (!body || typeof body !== "object" || Array.isArray(body))
      )
        return response(fail(400, "validation", "body must be an object"));
      const routeNeedsAuth =
        path === "/me" ||
        path === "/projects" ||
        path.startsWith("/projects/") ||
        path.startsWith("/tasks/");
      if (routeNeedsAuth && !user) return response(fail(401, "unauthorized"));
      if (user) {
        const min = Math.floor(s.now() / 60000),
          v = s.limits.get(token!) || { minute: min, count: 0 };
        if (v.minute !== min) {
          v.minute = min;
          v.count = 0;
        }
        v.count++;
        s.limits.set(token!, v);
        if (v.count > 100)
          return response({
            ...fail(429, "rate_limited"),
            headers: {
              "Retry-After": String(
                Math.ceil((60000 - (s.now() % 60000)) / 1000),
              ),
              "X-RateLimit-Limit": "100",
              "X-RateLimit-Remaining": "0",
            },
          });
      }
      let r: Result | null =
        userRoute(ctx, path, m) || projectRoute(ctx, path, m);
      if (!r && path.match(/^\/projects\/\d+\/tasks$/) && m === "GET") {
        const p = Number(path.split("/")[2]);
        r = projectRoute(ctx, `/projects/${p}`, "GET");
        if (r?.status === 200) r = listRoute(ctx, p);
      }
      if (!r && path.match(/^\/projects\/\d+\/tasks\/bulk$/) && m === "POST")
        r = bulk(ctx);
      if (!r) r = taskRoute(ctx, path, m);
      if (!r && path.match(/^\/projects\/\d+\/stats$/) && m === "GET")
        r = stats(ctx);
      if (!r) r = fail(404, "not_found");
      if (user) {
        r.headers = {
          ...(r.headers || {}),
          "X-RateLimit-Limit": "100",
          "X-RateLimit-Remaining": String(
            Math.max(0, 100 - (s.limits.get(token!)?.count || 1)),
          ),
        };
      }
      return response(r);
    },
  };
  function bulk(c: Ctx): Result {
    const p = Number(c.url.pathname.split("/")[2]);
    if (
      !c.state.projects.some((x) => x.id === p) ||
      !c.state.members.get(p)?.has(c.user!.id)
    )
      return fail(404, "not_found");
    if (c.state.members.get(p)?.get(c.user!.id) === "viewer")
      return fail(403, "forbidden");
    if (
      !Array.isArray(c.body?.tasks) ||
      c.body.tasks.length < 1 ||
      c.body.tasks.length > 50
    )
      return fail(400, "validation", "invalid tasks", "tasks");
    const made = [];
    const taskCount = c.state.tasks.length;
    const ids = { ...c.state.ids };
    for (let i = 0; i < c.body.tasks.length; i++) {
      const r = createTask(c, p, c.body.tasks[i]);
      if (r.status !== 201) {
        const added = c.state.tasks.splice(taskCount);
        for (const task of added) {
          c.state.histories.delete(task.id);
          c.state.comments.delete(task.id);
          c.state.deps.delete(task.id);
        }
        c.state.ids = ids;
        const field = r.body?.error?.field || "tasks";
        return fail(
          r.status,
          r.body.error.code,
          r.body.error.message,
          `tasks[${i}].${field}`,
        );
      }
      made.push(r.body);
    }
    return { status: 201, body: { items: made } };
  }
  function stats(c: Ctx): Result {
    const p = Number(c.url.pathname.split("/")[2]);
    if (
      !c.state.projects.some((x) => x.id === p) ||
      !c.state.members.get(p)?.has(c.user!.id)
    )
      return fail(404, "not_found");
    const ts = c.state.tasks.filter((t) => t.projectId === p),
      count = (field: string, values: string[]) =>
        Object.fromEntries(
          values.map((v) => [
            v,
            ts.filter((t) => (t as any)[field] === v).length,
          ]),
        );
    const assignee = new Map<number, number>();
    for (const t of ts)
      if (t.assigneeId !== null)
        assignee.set(t.assigneeId, (assignee.get(t.assigneeId) || 0) + 1);
    return {
      status: 200,
      body: {
        total: ts.length,
        byStatus: count("status", ["open", "in_progress", "done"]),
        byPriority: count("priority", ["high", "normal", "low"]),
        byAssignee: [...assignee]
          .map(([userId, count]) => ({ userId, count }))
          .sort((a, b) => b.count - a.count || a.userId - b.userId),
      },
    };
  }
  function response(r: Result) {
    const h = new Headers(r.headers);
    if (r.status === 204)
      return new Response(null, { status: 204, headers: h });
    return Response.json(r.body ?? null, { status: r.status, headers: h });
  }
}
function allowed(path: string): string[] | null {
  if (path === "/users") return ["POST"];
  if (path === "/sessions") return ["POST", "DELETE"];
  if (path === "/me") return ["GET"];
  if (path === "/projects") return ["GET", "POST"];
  if (/^\/projects\/\d+$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/tasks(?:\/bulk)?$/.test(path))
    return path.endsWith("/bulk") ? ["POST"] : ["GET", "POST"];
  if (/^\/projects\/\d+\/members(?:\/\d+)?$/.test(path))
    return path.endsWith("/members") ? ["GET", "POST"] : ["DELETE"];
  if (/^\/projects\/\d+\/stats$/.test(path)) return ["GET"];
  if (/^\/tasks\//.test(path)) return ["GET", "PATCH", "DELETE", "POST"];
  return null; }