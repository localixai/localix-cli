import { auth, body, fail, json, validate, type Ctx } from "./util";
import { randomUUID } from "node:crypto";
export async function users(c: Ctx, method: string, path: string): Promise<Response | undefined> {
  const s = c.state;
  if (path === "/users" && method === "POST") {
    const b = await body(c.request);
    validate(typeof b.email === "string" && /^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(b.email), "email");
    validate(typeof b.password === "string" && b.password.length >= 8, "password");
    validate(typeof b.name === "string" && b.name.trim().length > 0 && b.name.trim().length <= 100, "name");
    const email = b.email.toLowerCase();
    if (s.users.some((u) => u.email === email)) fail(409, "conflict", "Email already exists", "email");
    const user = { id: s.counters.user = (s.counters.user || 0) + 1,
      email, password: b.password, name: b.name.trim() };
    s.users.push(user);
    return json({ id: user.id, email, name: user.name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const b = await body(c.request); const email = typeof b.email === "string" ? b.email.toLowerCase() : "";
    const user = s.users.find((u) => u.email === email && u.password === b.password);
    if (!user) fail(401, "unauthorized", "Invalid email or password");
    const token = randomUUID(); s.tokens.set(token, user!.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const { token } = auth(c); s.tokens.delete(token!); return new Response(null, { status: 204 });
  }
  if (path === "/me" && method === "GET") {
    const u = auth(c).user!; return json({ id: u.id, email: u.email, name: u.name });
  }
}
