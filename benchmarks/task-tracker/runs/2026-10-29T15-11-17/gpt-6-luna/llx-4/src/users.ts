import type { Store } from "./model";
import { body, error, json, auth } from "./http";

export async function users(req: Request, s: Store, path: string[]): Promise<Response> {
  if (path[0] === "me" && req.method === "GET") {
    const u = auth(req, s);
    if (!u) return error("unauthorized", "unauthorized", 401);
    const lim = s.limits.get(req.headers.get("Authorization")!);
    const minute = Math.floor(s.now() / 60000);
    if (!lim || lim.minute !== minute) s.limits.set(req.headers.get("Authorization")!, { minute, count: 0 });
    const cur = s.limits.get(req.headers.get("Authorization")!)!;
    if (cur.count >= 100) {
      const limited = error("rate_limited", "rate limited", 429);
      limited.headers.set("Retry-After", String(Math.ceil(((minute + 1) * 60000 - s.now()) / 1000)));
      limited.headers.set("X-RateLimit-Limit", "100");
      limited.headers.set("X-RateLimit-Remaining", "0");
      return limited;
    }
    cur.count++;
    return json(publicUser(u), 200, { "X-RateLimit-Limit": "100",
      "X-RateLimit-Remaining": String(100 - cur.count) });
  }
  if (path[0] === "sessions" && req.method === "DELETE") {
    const token = /^Bearer (.+)$/.exec(req.headers.get("Authorization") || "")?.[1];
    if (!token || !s.tokens.has(token)) return error("unauthorized", "unauthorized", 401);
    s.tokens.delete(token);
    return new Response(null, { status: 204 });
  }
  if (req.method !== "POST") return error("method_not_allowed", "method not allowed", 405);
  const b = await body(req);
  if (!b) return error("validation", "invalid body");
  if (path[0] === "users") {
    const email = typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
    const name = typeof b.name === "string" ? b.name.trim() : "";
    if (!/^[^@]+@[^@]+\.[^@]+$/.test(email)) return error("validation", "invalid email", 400, "email");
    if (typeof b.password !== "string" || b.password.length < 8)
      return error("validation", "invalid password", 400, "password");
    if (!name || name.length > 100) return error("validation", "invalid name", 400, "name");
    if (s.users.some(u => u.email === email)) return error("conflict", "email taken", 409, "email");
    const u = { id: s.users.length + 1, email, name, password: b.password };
    s.users.push(u);
    return json(publicUser(u), 201);
  }
  if (path[0] === "sessions") {
    const email = typeof b.email === "string" ? b.email.toLowerCase() : "";
    const u = s.users.find(x => x.email === email && x.password === b.password);
    if (!u) return error("unauthorized", "unauthorized", 401);
    const token = `${u.id}-${s.now()}-${Math.random()}`;
    s.tokens.set(token, u.id);
    return json({ token }, 201);
  }
  return error("not_found", "not found", 404);
}
function publicUser(u: { id: number; email: string; name: string }) {
  return { id: u.id, email: u.email, name: u.name };
}
