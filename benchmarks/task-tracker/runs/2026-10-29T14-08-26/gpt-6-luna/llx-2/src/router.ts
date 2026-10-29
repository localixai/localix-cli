import type { Store, User } from "./model";
import { bad, fail, obj, reply } from "./http";
import { domain } from "./domain";
const userView = (u: User) => ({ id: u.id, email: u.email, name: u.name });
const authorized = (s: Store, request: Request) => {
  const auth = request.headers.get("Authorization") ?? "";
  const token = auth.startsWith("Bearer ") ? auth.slice(7) : "";
  const id = s.sessions.get(token);
  if (!id) fail(401, "unauthorized", "Authentication required");
  const minute = Math.floor(s.now() / 60000);
  let limit = s.limits.get(token);
  if (!limit || limit.minute !== minute) limit = { minute, count: 0 };
  limit.count++;
  s.limits.set(token, limit);
  const remaining = Math.max(0, 100 - limit.count);
  const headers = { "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": String(remaining) };
  if (limit.count > 100) {
    const retry = Math.ceil(((minute + 1) * 60000 - s.now()) / 1000);
    return { id, headers: { ...headers, "Retry-After": String(retry) }, limited: true };
  }
  return { id, headers, limited: false };
};
export async function route(s: Store, request: Request): Promise<Response> {
  const url = new URL(request.url), path = url.pathname, method = request.method;
  if (path === "/users" && method === "POST") {
    const b = await obj(request);
    const email = typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
    const name = typeof b.name === "string" ? b.name.trim() : "";
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)) bad("email");
    if (typeof b.password !== "string" || b.password.length < 8) bad("password");
    if (!name || name.length > 100) bad("name");
    if (s.users.some((u) => u.email === email)) fail(409, "conflict", "Email is taken", "email");
    const u = { id: s.users.length + 1, email, name, password: b.password };
    s.users.push(u); return reply(userView(u), 201);
  }
  if (path === "/sessions" && method === "POST") {
    const b = await obj(request), u = s.users.find((x) =>
      x.email === String(b.email ?? "").toLowerCase() && x.password === b.password);
    if (!u) fail(401, "unauthorized", "Invalid email or password");
    const token = `session-${++s.tokenSeq}-${s.now()}`;
    s.sessions.set(token, u.id); return reply({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const a = authorized(s, request); if (a.limited) return reply({ error: { code: "rate_limited",
      message: "Rate limit exceeded" } }, 429, a.headers);
    s.sessions.delete(request.headers.get("Authorization")!.slice(7));
    return reply(null, 204, a.headers);
  }
  const known = path === "/me" || path === "/users" || path === "/sessions" ||
    path === "/projects" || /^\/projects\/[^/]+/.test(path) ||
    /^\/tasks\/[^/]+/.test(path);
  if (!known) fail(404, "not_found", "Not found");
  const methods = path === "/users" ? ["POST"] : path === "/sessions" ? ["POST", "DELETE"] :
    path === "/me" ? ["GET"] : path === "/projects" ? ["GET", "POST"] : null;
  if (methods && !methods.includes(method)) return reply({ error: { code: "method_not_allowed",
    message: "Method not allowed" } }, 405, { Allow: methods.join(", ") });
  const allowed = routes[path] ?? null;
  if (allowed && !allowed.includes(method)) return reply({ error: { code: "method_not_allowed",
    message: "Method not allowed" } }, 405, { Allow: allowed.join(", ") });
  const auth = authorized(s, request);
  if (auth.limited) return reply({ error: { code: "rate_limited", message: "Rate limit exceeded" } },
    429, auth.headers);
  const uid = auth.id;
    const response = await dispatch(s, request, url, uid);
  response.headers.set("X-RateLimit-Limit", "100");
  response.headers.set("X-RateLimit-Remaining", auth.headers["X-RateLimit-Remaining"]);
  return response;
}
const routes: Record<string, string[]> = { "/me": ["GET"] };
async function dispatch(s: Store, r: Request, u: URL, uid: number): Promise<Response> {
  const path = u.pathname, method = r.method;
  if (path === "/me" && method === "GET") {
    const user = s.users.find((x) => x.id === uid)!; return reply(userView(user));
  }
  return await domain(s, r, u, uid);
}
