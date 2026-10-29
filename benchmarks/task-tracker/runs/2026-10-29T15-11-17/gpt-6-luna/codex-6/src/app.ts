import { state, type Context } from "./model";
import { authenticate, rateHeaders, session, signup, tokenFrom } from "./users";
import { empty, errorResponse, fail, ok, read } from "./http";
import { projectView, visibleProject } from "./projects";
import { createProject } from "./projects";
import { byKey } from "./taskops";
import { projectRoute, taskRoute } from "./routes";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
const routes: Record<string, string[]> = {
  "/users": ["POST"], "/sessions": ["POST", "DELETE"], "/me": ["GET"],
  "/projects": ["GET", "POST"], "/projects/:id": ["GET"],
  "/projects/:id/members": ["GET", "POST"], "/projects/:id/members/:uid": ["DELETE"],
  "/projects/:id/tasks": ["GET", "POST"], "/projects/:id/tasks/bulk": ["POST"],
  "/projects/:id/stats": ["GET"], "/tasks/:key": ["GET", "PATCH", "DELETE"],
  "/tasks/:key/comments": ["GET", "POST"], "/tasks/:key/history": ["GET"],
  "/tasks/:key/transitions": ["POST"], "/tasks/:key/dependencies": ["GET", "POST"],
  "/tasks/:key/dependencies/:dep": ["DELETE"],
};
function match(path: string) {
  for (const [pattern, methods] of Object.entries(routes)) {
    const names: string[] = [];
    const regex = new RegExp(`^${pattern.replace(/:[^/]+/g, (m) => {
      names.push(m.slice(1)); return "([^/]+)";
    })}$`);
    const found = regex.exec(path);
    if (found) return { names, values: found.slice(1), methods };
  }
}
export function createApp(options: Options = {}): App {
  const c: Context = { s: state(), now: options.now ?? Date.now };
  return { async fetch(request) {
    const url = new URL(request.url); const route = match(url.pathname);
    try {
      if (!route) fail(404, "not_found", "Route not found");
      const method = request.method.toUpperCase();
      if (!route!.methods.includes(method)) fail(405, "method_not_allowed", "Method not allowed");
      const params = Object.fromEntries(route!.names.map((n, i) => [n, decodeURIComponent(route!.values[i]!)]));
      let user;
      if (!(url.pathname === "/users" || (url.pathname === "/sessions" && method === "POST"))) {
        user = authenticate(c, request);
      }
      let result: any; let status = 200; let headers: Record<string, string> = {};
      let replayed = false;
      if (url.pathname === "/users") { result = signup(c, await read(request)); status = 201; }
      else if (url.pathname === "/sessions" && method === "POST") {
        result = session(c, await read(request)); status = 201;
      } else if (url.pathname === "/sessions") { c.s.tokens.delete(tokenFrom(request)!); return empty(); }
      else if (url.pathname === "/me") result = { id: user!.id, email: user!.email, name: user!.name };
      else if (url.pathname === "/projects" && method === "POST") {
        result = createProject(c, user!, await read(request)); status = 201;
      } else if (url.pathname === "/projects") {
        result = c.s.projects.filter((p) => c.s.members.get(p.id)?.has(user!.id)).map(projectView);
      } else if (params.id) {
        const id = /^\d+$/.test(params.id) ? Number(params.id) : -1;
        const p = visibleProject(c, id, user!.id)!;
        const idemKey = request.headers.get("Idempotency-Key");
        const prior = idemKey ? c.s.idem.get(`${user!.id}:${idemKey}`) : undefined;
        replayed = !!prior && c.now() - prior.at < 86400000;
        result = await projectRoute(c, request, url, p, user!, params, method);
        if (method === "POST") status = url.pathname.endsWith("/members") ||
          url.pathname.endsWith("/tasks") || url.pathname.endsWith("/bulk") ? 201 : 200;
        if (method === "DELETE") status = 204;
      } else if (params.key) {
        const { t, p } = byKey(c, params.key, user!.id)!!;
        const reply = (await taskRoute(c, request, t!, p, user!, params, method))!;
        result = reply.result; status = reply.status ?? 200; headers = reply.headers ?? {};
      } else fail(404, "not_found", "Route not found");
      if (url.pathname.startsWith("/tasks/") && (method === "POST") &&
        (url.pathname.endsWith("/comments") || url.pathname.endsWith("/dependencies"))) status = 201;
      if (method === "POST" && url.pathname.includes("/transitions")) headers.ETag = `"${result.version}"`;
      if (method === "POST" && url.pathname.endsWith("/tasks")) headers.ETag = `"${result.version}"`;
      if (replayed) headers["Idempotent-Replayed"] = "true";
      const response = status === 204 ? empty() : ok(result, status, headers);
      const token = tokenFrom(request);
      for (const [key, value] of Object.entries(rateHeaders(c, token))) response.headers.set(key, value);
      return response;
    } catch (error) {
      const response = errorResponse(error);
      const token = tokenFrom(request);
      for (const [key, value] of Object.entries(rateHeaders(c, token))) response.headers.set(key, value);
      if ((error as any)?.status === 405 && route) response.headers.set("Allow", route!.methods.join(", "));
      if ((error as any)?.status === 429) {
        const retry = Math.ceil((Math.floor(c.now() / 60000) * 60000 + 60000 - c.now()) / 1000);
        response.headers.set("Retry-After", String(retry));
        response.headers.set("X-RateLimit-Remaining", "0");
      }
      return response;
    }
  } };
}
