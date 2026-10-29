import type { State } from "./model";
import { accounts } from "./accounts";
import { projects } from "./projects";
import { tasks } from "./tasks";
import { list, comments, dependencies } from "./taskextra";
import { stats } from "./stats";
import { allow, errorResponse, rate, fail } from "./http";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
const methods: Record<string, string[]> = {
  users: ["POST"], sessions: ["POST", "DELETE"], me: ["GET"], projects: ["GET", "POST"],
};
function allowed(path: string[]) {
  if (path.length === 1 && path[0]! in methods) return methods[path[0]!];
  if (path[0] === "projects" && path.length === 2) return ["GET"];
  if (path[0] === "projects" && path[2] === "members") return path.length === 3 ? ["GET", "POST"] : ["DELETE"];
  if (path[0] === "projects" && path[2] === "tasks") return path.length === 3 ? ["GET", "POST"] : ["POST"];
  if (path[0] === "projects" && path[2] === "stats") return ["GET"];
  if (path[0] === "tasks") {
    if (path.length === 2) return ["GET", "PATCH", "DELETE"];
    if (path[2] === "transitions") return ["POST"];
    if (path[2] === "history") return ["GET"];
    if (path[2] === "comments") return path.length === 3 ? ["GET", "POST"] : [];
    if (path[2] === "dependencies") return path.length === 3 ? ["GET", "POST"] : ["DELETE"];
  }
  return undefined;
}
export function createApp(options: Options = {}): App {
  const s: State = {
    users: [], projects: [], tasks: [], tokens: new Map(), members: new Map(), comments: new Map(),
    histories: new Map(), deps: new Map(), idem: new Map(), limits: new Map(), now: options.now ?? Date.now,
    ids: { user: 0, project: 0, task: 0, comment: 0, token: 0 },
  };
  return { async fetch(request) {
    try {
      const path = new URL(request.url).pathname.split("/").filter(Boolean).map(decodeURIComponent);
      const allowList = allowed(path);
      if (!allowList) fail(404, "not_found", "Route not found");
      if (!allowList.includes(request.method)) return allow(request.method, allowList);
      const headers = rate(request, s);
      const response = await accounts(request, path, s) ?? await projects(request, path, s) ??
        await tasks(request, path, s) ?? stats(request, path, s);
      if (!response) return errorResponse(new Error("not found"));
      if (Object.keys(headers).length) for (const [k, v] of Object.entries(headers)) response.headers.set(k, v);
      return response;
    } catch (error) {
      if (error instanceof Response) return error;
      return errorResponse(error);
    }
  } };
}
