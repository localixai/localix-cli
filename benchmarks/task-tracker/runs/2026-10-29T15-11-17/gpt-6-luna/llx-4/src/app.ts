import { Store } from "./model";
import { auth, error } from "./http";
import { users } from "./users";
import { projects } from "./projects";
import { tasks } from "./tasks";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const s: Store = {
    now: options.now || Date.now, users: [], projects: [], tasks: [],
    tokens: new Map(), comments: new Map(), history: new Map(), limits: new Map(),
    idempotency: new Map(), nextComment: 1,
  };
  return { fetch: req => dispatch(req, s) };
}

async function dispatch(req: Request, s: Store): Promise<Response> {
  const url = new URL(req.url), path = url.pathname.split("/").filter(Boolean);
  const methods: Record<string, string[]> = {
    "/users": ["POST"], "/sessions": ["POST", "DELETE"], "/me": ["GET"],
    "/projects": ["GET", "POST"],
  };
  if (methods[url.pathname] && !methods[url.pathname]!.includes(req.method)) {
    const response = error("method_not_allowed", "method not allowed", 405);
    response.headers.set("Allow", methods[url.pathname]!.join(", "));
    return response;
  }
  if (path[0] === "users" || path[0] === "sessions" || path[0] === "me")
    return users(req, s, path);
  const user = auth(req, s);
  if (req.method !== "POST" && req.method !== "GET" && req.method !== "PATCH" &&
      req.method !== "DELETE") return error("method_not_allowed", "method not allowed", 405);
  if (path[0] === "projects" && path.length === 1) {
    if (!user) return error("unauthorized", "unauthorized", 401);
    if (req.method === "GET") return projects(req, s, path, user);
    if (req.method === "POST") return projects(req, s, path, user);
    return error("method_not_allowed", "method not allowed", 405);
  }
  if (path[0] === "projects" || path[0] === "tasks") {
    if (!user) return error("unauthorized", "unauthorized", 401);
    return path[0] === "projects"
      ? projects(req, s, path, user) : tasks(req, s, path, user, url);
  }
  return error("not_found", "not found", 404);
}
