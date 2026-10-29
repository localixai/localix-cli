import { State, error } from "./model";
import { userRoutes } from "./users";
import { projectRoutes } from "./projects";
import { taskRoutes } from "./task-routes";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
const methods: Record<string,string> = {
  "/users":"POST", "/sessions":"POST, DELETE", "/me":"GET", "/projects":"GET, POST",
};
export function createApp(options: Options = {}): App {
  const s: State = { now: options.now || Date.now, users: [], projects: [], tasks: [], members: new Map(),
    tokens: new Map(), comments: new Map(), history: new Map(), deps: new Map(), limits: new Map(),
    idem: new Map(), taskCounters: new Map(), uid: 0, pid: 0, tid: 0, tokenId: 0 };
  return { async fetch(request: Request) {
    const url = new URL(request.url), method = request.method, path = url.pathname;
    const allowed = methods[path];
    if (allowed && !allowed.split(", ").includes(method)) {
      return new Response(JSON.stringify({ error: { code:"method_not_allowed", message:"Method not allowed" } }),
        { status:405, headers:{ "Content-Type":"application/json", Allow:allowed } });
    }
    const auth = request.headers.get("Authorization") || "";
    const token = auth.startsWith("Bearer ") ? auth.slice(7) : "";
    let uid = s.tokens.get(token);
    if (!allowed && !path.startsWith("/projects/") && !path.startsWith("/tasks/")) {
      return error("not_found", "Not found", 404);
    }
    const publicRoute = path === "/users" && method === "POST" ||
      path === "/sessions" && method === "POST";
    if (!publicRoute) {
      if (!uid) return error("unauthorized","Unauthorized",401);
      const windowStart = Math.floor(s.now()/60000)*60000;
      let limit = s.limits.get(token);
      if (!limit || limit.start !== windowStart) { limit={start:windowStart,count:0}; s.limits.set(token,limit); }
      if (limit.count >= 100) {
        const retry = Math.ceil((windowStart+60000-s.now())/1000);
        const response = error("rate_limited","Rate limit exceeded",429);
        response.headers.set("Retry-After",String(retry)); response.headers.set("X-RateLimit-Limit","100");
        response.headers.set("X-RateLimit-Remaining","0"); return response;
      }
      limit.count++;
    }
    let body: any = {};
    if (request.body && !["GET","HEAD"].includes(method)) {
      try { body=await request.json(); } catch { return error("validation","Invalid JSON",400); }
      if (!body || typeof body !== "object" || Array.isArray(body)) {
        return error("validation", "Body must be an object", 400);
      }
    }
    if (path.startsWith("/projects") || path.startsWith("/tasks")) {
      if (!uid) return error("unauthorized","Unauthorized",401);
      const response = taskRoutes(s,method,path,url,body,uid,request.headers) ||
        projectRoutes(s,method,path,body,uid) || error("not_found","Not found",404);
      if (token && response.status !== 429) {
        response.headers.set("X-RateLimit-Limit","100");
        response.headers.set("X-RateLimit-Remaining",String(100-(s.limits.get(token)?.count || 0)));
      }
      return response;
    }
    const response = userRoutes(s,method,path,body,uid) || error("not_found","Not found",404);
    if (token && response.status !== 429) {
      response.headers.set("X-RateLimit-Limit","100");
      response.headers.set("X-RateLimit-Remaining",String(100-(s.limits.get(token)?.count || 0)));
    }
    return response;
  } };
}
