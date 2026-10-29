import { type State } from "./model";
import { Fault, type Ctx, ok } from "./http";
import { people } from "./people";
import { projects } from "./projects";
import { taskLists } from "./task-list";
import { taskReadWrite } from "./task-core";
import { taskExtra } from "./task-extra";
import { taskDelete } from "./task-delete";
import { stats } from "./stats";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const s: State = {
    users: [], projects: [], tasks: [], members: new Map(), sessions: new Map(),
    comments: new Map(), history: new Map(), idem: new Map(), counts: new Map(),
    nextTask: new Map(),
    now: options.now ?? Date.now, ids: { user: 0, project: 0, task: 0, token: 0 },
  };
  return { fetch: (request) => dispatch(s, request) };
}
async function dispatch(s: State, request: Request): Promise<Response> {
  const url = new URL(request.url), path = url.pathname, method = request.method;
  const routes: Record<string, string[]> = {
    "/users": ["POST"], "/sessions": ["POST", "DELETE"], "/me": ["GET"],
    "/projects": ["GET", "POST"],
  };
  const exact = path.match(/^\/(users|sessions|me|projects)$/);
  if (exact && !routes[path]?.includes(method)) {
    const allowed = routes[path]!.join(", ");
    return error(405, "method_not_allowed", "Method not allowed", undefined, { Allow: allowed });
  }
  const token = request.headers.get("Authorization")?.match(/^Bearer (.+)$/)?.[1];
  const user = token ? s.sessions.get(token) ?? null : null;
  if (token && user) {
    const minute = Math.floor(s.now() / 60000), prev = s.counts.get(token);
    const entry = prev?.minute === minute ? prev : { minute, count: 0 };
    if (entry.count >= 100) {
      const retry = Math.ceil(((minute + 1) * 60000 - s.now()) / 1000);
      return error(429, "rate_limited", "Rate limit exceeded", undefined, {
        "Retry-After": String(retry), "X-RateLimit-Limit": "100",
        "X-RateLimit-Remaining": "0",
      });
    }
    entry.count++; s.counts.set(token, entry);
  }
  const c: Ctx = { s, request, url, user };
  try {
    let response: Response | null = await people(c, path, method);
    response ??= await projects(c, path, method);
    response ??= stats(c, path, method);
    response ??= await taskLists(c, path, method);
    response ??= await taskExtra(c, path, method);
    response ??= await taskReadWrite(c, path, method);
    response ??= taskDelete(c, path, method);
    if (!response) return error(404, "not_found", "Not found");
    if (token && user) {
      const remaining = Math.max(0, 100 - (s.counts.get(token)?.count ?? 0));
      response.headers.set("X-RateLimit-Limit", "100");
      response.headers.set("X-RateLimit-Remaining", String(remaining));
    }
    return response;
  } catch (e) {
    if (e instanceof Fault) return error(e.status, e.code, e.message, e.field);
    return error(500, "internal_error", "Internal error");
  }
}
function error(status: number, code: string, message: string, field?: string,
  headers: Record<string, string> = {}): Response {
  const detail: Record<string, string> = { code, message };
  if (field) detail.field = field;
  return ok({ error: detail }, status, headers);
}
