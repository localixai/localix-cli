import { createState } from "./model";
import { errorResponse, json } from "./http";
import { usersRoute } from "./users";
import { projectsRoute } from "./projects";
import { taskRoute } from "./tasks";
import { createTasks, listTasks } from "./task-create";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
const methods: Record<string, string> = {
  "/users": "POST", "/sessions": "POST, DELETE", "/me": "GET", "/projects": "GET, POST",
};
function withLimits(response: Response, request: Request): Response {
  const remaining = request.headers.get("X-Internal-Remaining");
  if (remaining === null) return response;
  const headers = new Headers(response.headers);
  headers.set("X-RateLimit-Limit", "100");
  headers.set("X-RateLimit-Remaining", remaining);
  return new Response(response.body, { status: response.status, headers });
}
export function createApp(options: Options = {}): App {
  const state = createState(options.now ?? Date.now);
  return { async fetch(request: Request) {
    const url = new URL(request.url);
    const method = request.method.toUpperCase();
    try {
      const user = await usersRoute(state, request, url.pathname, method);
      if (user) return withLimits(user, request);
      const project = await projectsRoute(state, request, url.pathname, method, url);
      if (project) return withLimits(project, request);
      const task = await taskRoute(state, request, url.pathname, method);
      if (task) return withLimits(task, request);
      const allow = methods[url.pathname];
      if (allow) {
        return json({ error: { code: "method_not_allowed", message: "Method not allowed" } },
          405, { Allow: allow });
      }
      return json({ error: { code: "not_found", message: "Not found" } }, 404);
    } catch (error) {
      if (error instanceof Response) return error;
      return errorResponse(error);
    }
  } };
}
