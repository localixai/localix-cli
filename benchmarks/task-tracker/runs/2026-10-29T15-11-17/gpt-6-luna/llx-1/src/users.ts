import type { RequestCtx } from "./http";
import { bad, body, conflict, empty, err, ok } from "./http";

export async function users(method: string, path: string, ctx: RequestCtx) {
  const { state: s, req } = ctx;
  if (path === "/users" && method === "POST") {
    const b = await body(req);
    if (!b) return err(400,"validation","Invalid JSON");
    if (typeof b.email !== "string" || !/^[^@]+@[^@]+\.[^@]+$/.test(b.email)) return bad("email");
    if (typeof b.password !== "string" || b.password.length < 8) return bad("password");
    if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) return bad("name");
    const email = b.email.toLowerCase();
    if (s.users.some(u => u.email === email)) return conflict("email");
    const user = { id: ++s.ids.user, email, name: b.name.trim(), password: b.password };
    s.users.push(user);
    return ok({ id: user.id, email, name: user.name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const b = await body(req); if (!b) return err(400,"validation","Invalid JSON");
    const u = s.users.find(x => x.email === String(b.email).toLowerCase() && x.password === b.password);
    if (!u) return err(401, "unauthorized", "Unauthorized");
    const token = `session-${u.id}-${crypto.randomUUID()}`;
    s.tokens.set(token, u.id);
    return ok({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    for (const [t, id] of s.tokens) if (t === req.headers.get("Authorization")?.slice(7)) s.tokens.delete(t);
    return empty();
  }
  if (path === "/me" && method === "GET") {
    const u = s.users.find(x => x.id === ctx.userId)!;
    return ok({ id: u.id, email: u.email, name: u.name });
  }
  return undefined;
}
