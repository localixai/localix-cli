import type { Ctx } from "./types";
import { body, conflict, fail, json, malformed, validation } from "./util";

export async function people(ctx: Ctx): Promise<Response | null> {
  const { request: req, store, user } = ctx;
  if (ctx.url.pathname === "/users" && req.method === "POST") {
    const b = await body(req);
    if (!b) return malformed();
    const email = typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
    const name = typeof b.name === "string" ? b.name.trim() : "";
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)) return validation("email");
    if (typeof b.password !== "string" || b.password.length < 8) return validation("password");
    if (!name || name.length > 100) return validation("name");
    if (store.users.some(x => x.email === email)) return conflict("email");
    const u = { id: store.users.length + 1, email, name, password: b.password };
    store.users.push(u);
    return json({ id: u.id, email, name }, 201);
  }
  if (ctx.url.pathname === "/sessions" && req.method === "POST") {
    const b = await body(req);
    if (!b) return malformed();
    const u = store.users.find(x => x.email === String(b.email ?? "").toLowerCase() &&
      x.password === b.password);
    if (!u) return fail("unauthorized", "Unauthorized", 401);
    const token = `token-${store.nextToken++}`;
    store.tokens.set(token, u.id);
    return json({ token }, 201);
  }
  if (ctx.url.pathname === "/sessions" && req.method === "DELETE") {
    const token = req.headers.get("Authorization")?.replace(/^Bearer /, "");
    if (token) store.tokens.delete(token);
    return new Response(null, { status: 204 });
  }
  if (ctx.url.pathname === "/me" && req.method === "GET" && user)
    return json({ id: user.id, email: user.email, name: user.name });
  return null;
}
