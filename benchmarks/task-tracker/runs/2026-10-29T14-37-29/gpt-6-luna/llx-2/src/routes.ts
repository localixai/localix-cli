import type { State } from "./state";
import { body, error } from "./http";
import { users } from "./users";
import { projects } from "./projects";
import { tasks } from "./tasks";

export async function handle(request: Request, state: State): Promise<Response> {
  const url = new URL(request.url);
  const method = request.method;
  const path = url.pathname;
  const allowed: Record<string, string> = {
    "/users": "POST", "/sessions": "POST, DELETE", "/me": "GET",
    "/projects": "GET, POST",
  };
  if (allowed[path] && !allowed[path].split(", ").includes(method)) {
    return new Response(JSON.stringify({
      error: { code: "method_not_allowed", message: "Method not allowed" },
    }), { status: 405, headers: { Allow: allowed[path], "Content-Type": "application/json" } });
  }
  const token = request.headers.get("Authorization") ?? "";
  if (token.startsWith("Bearer ") && state.sessions.has(token.slice(7))) {
    const value = state.sessions.get(token.slice(7))!;
    const minute = Math.floor(state.now() / 60000);
    let bucket = state.limits.get(token.slice(7));
    if (!bucket || bucket.minute !== minute) bucket = { minute, count: 0 };
    bucket.count++;
    state.limits.set(token.slice(7), bucket);
    const headers = {
      "X-RateLimit-Limit": "100",
      "X-RateLimit-Remaining": String(Math.max(0, 100 - bucket.count)),
    };
    void value;
    if (bucket.count > 100) return Response.json(
      { error: { code: "rate_limited", message: "Rate limit exceeded" } },
      { status: 429, headers: { ...headers, "Retry-After": String(
        Math.ceil(((minute + 1) * 60000 - state.now()) / 1000),
      ) } },
    );
    // Rate headers are added to all authenticated replies below.
    void headers;
  }
  const parsed = await body(request);
  if (["POST", "PATCH", "PUT"].includes(method) && request.headers.has("content-type") && !parsed) {
    return error("validation", "Body must be a JSON object", 400);
  }
  const context = { request, state, url, method, path, input: parsed };
  const reply = await users(context) ?? await projects(context) ?? await tasks(context);
  if (reply) {
    if (token.startsWith("Bearer ") && state.sessions.has(token.slice(7))) {
      const bucket = state.limits.get(token.slice(7))!;
      reply.headers.set("X-RateLimit-Limit", "100");
      reply.headers.set("X-RateLimit-Remaining", String(Math.max(0, 100 - bucket.count)));
    }
    return reply;
  }
  return error("not_found", "Not found", 404);
}
