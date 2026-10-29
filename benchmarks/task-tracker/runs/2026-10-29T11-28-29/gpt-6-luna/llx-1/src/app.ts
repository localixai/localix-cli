import { account } from "./accounts";
import { auth, Fault, fail } from "./common";
import { projects } from "./projects";
import type { State } from "./model";
import { create, taskRoute } from "./tasks";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const s: State = { users: [], projects: [], tasks: [], members: new Map(), tokens: new Map(),
    counters: new Map(), limits: new Map(), idem: new Map(), now: options.now ?? Date.now,
    nextUser: 1, nextProject: 1, nextTask: 1, nextComment: 1, nextToken: 1 };
  return { async fetch(req) {
    const url = new URL(req.url), path = url.pathname;
    const allowed = methods(path);
    if (allowed && !allowed.includes(req.method)) return response(405, error("method_not_allowed"), { Allow: allowed.join(", ") });
    try {
      const a = await account(s, req, path);
      if (a) return response(a[0] as number, a[1]);
      if (/^\/projects\/\d+\/tasks(?:\/bulk)?$/.test(path) && req.method === "POST") {
        const user = auth(s, req), pid = Number(path.split("/")[2]);
        if (path.endsWith("/bulk")) return response(404, error("not_found"));
        const p = s.projects.find(x => x.id === pid);
        if (!p || !s.members.get(pid)?.has(user)) fail(404, "not_found");
        const body = await req.json().catch(() => fail(400, "validation"));
        const key = req.headers.get("Idempotency-Key");
        if (key) {
          const id = `${user}:${key}`, prior = s.idem.get(id);
          if (prior && s.now() - prior.at < 86400000) {
            if (prior.body !== JSON.stringify(body)) fail(422, "idempotency_mismatch");
            return response(201, (await import("./model")).taskOut(prior.task), { "Idempotent-Replayed": "true", ETag: `"${prior.task.version}"` });
          }
          const t = create(s, pid, user, body as any);
          s.idem.set(id, { at: s.now(), body: JSON.stringify(body), task: t });
          return response(201, (await import("./model")).taskOut(t), { ETag: `"1"` });
        }
        const { taskOut } = await import("./model");
        const t = create(s, pid, user, body as any);
        return response(201, taskOut(t), { ETag: `"1"` });
      }
      const pr = await projects(s, req, url, path);
      if (pr) return response(pr[0] as number, pr[1]);
      let user: number;
      try { user = auth(s, req); } catch { throw new Fault(401, "unauthorized"); }
      const result = await taskRoute(s, req, url, path, user);
      if (result) {
        const body: any = result[1];
        return response(result[0] as number, body, body?.version ? { ETag: `"${body.version}"` } : {});
      }
      return response(404, error("not_found"));
    } catch (e) {
      if (e instanceof Fault) return response(e.status, { error: { code: e.code, message: e.message, ...(e.field ? { field: e.field } : {}) } });
      return response(400, error("validation"));
    }
  } };
}
function error(code: string) { return { error: { code, message: code.replaceAll("_", " ") } }; }
function response(status: number, body: unknown, headers: Record<string, string> = {}) {
  return new Response(status === 204 ? null : JSON.stringify(body), {
    status, headers: { ...(status === 204 ? {} : { "Content-Type": "application/json" }), ...headers },
  });
}
function methods(path: string): string[] | null {
  const known: Record<string, string[]> = {
    "/users": ["POST"], "/sessions": ["POST", "DELETE"], "/me": ["GET"],
    "/projects": ["GET", "POST"],
  };
  return known[path] ?? null;
}
