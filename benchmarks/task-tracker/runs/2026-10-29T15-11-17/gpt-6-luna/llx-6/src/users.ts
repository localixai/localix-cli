import { fail, ok, type Ctx, type Result } from "./http";
import { bad, type User } from "./model";

export function userRoutes(c: Ctx, path: string): Result | undefined {
  const s = c.state;
  if (path === "/users" && c.req.method === "POST") {
    const { email, password, name } = c.body ?? {};
    if (typeof email !== "string" || !/^[^@]+@[^@]+\.[^@]+$/.test(email))
      return { status: 400, body: bad("email") };
    if (typeof password !== "string" || password.length < 8)
      return { status: 400, body: bad("password") };
    if (typeof name !== "string" || !name.trim() || name.trim().length > 100)
      return { status: 400, body: bad("name") };
    const normalized = email.toLowerCase();
    if (s.users.some(u => u.email === normalized))
      return fail(409, "conflict", "Email already exists", "email");
    const u: User = { id: ++s.ids.user, email: normalized, password, name: name.trim() };
    s.users.push(u);
    return ok({ id: u.id, email: u.email, name: u.name }, 201);
  }
  if (path === "/sessions" && c.req.method === "POST") {
    const u = s.users.find(x => x.email === String(c.body?.email).toLowerCase() &&
      x.password === c.body?.password);
    if (!u) return fail(401, "unauthorized", "Invalid credentials");
    const token = `token-${u.id}-${s.now()}-${Math.random()}`;
    s.sessions.set(token, u.id);
    return ok({ token }, 201);
  }
  if (path === "/sessions" && c.req.method === "DELETE") {
    const token = c.req.headers.get("Authorization")?.replace(/^Bearer /, "");
    if (!c.user || !token) return fail(401, "unauthorized");
    s.sessions.delete(token);
    return { status: 204 };
  }
  if (path === "/me" && c.req.method === "GET") {
    const u = s.users.find(x => x.id === c.user);
    return u ? ok({ id: u.id, email: u.email, name: u.name }) : fail(401, "unauthorized");
  }
}
