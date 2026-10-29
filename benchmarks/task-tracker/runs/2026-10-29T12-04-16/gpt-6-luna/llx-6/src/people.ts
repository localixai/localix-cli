import { type Ctx, body, fail, noContent, ok, valid } from "./http";
import { iso } from "./model";
export async function people(c: Ctx, path: string, method: string): Promise<Response | null> {
  const { s } = c;
  if (path === "/users" && method === "POST") {
    const b = await body(c);
    valid(typeof b.email === "string" && /^[^@]+@[^@]+\.[^@]+$/.test(b.email), "email");
    valid(typeof b.password === "string" && b.password.length >= 8, "password");
    valid(typeof b.name === "string" && b.name.trim().length > 0 && b.name.trim().length <= 100, "name");
    const email = b.email.toLowerCase();
    if (s.users.some((u) => u.email === email)) fail(409, "conflict", "Email already exists", "email");
    const user = { id: ++s.ids.user, email, name: b.name.trim(), password: b.password };
    s.users.push(user);
    return ok({ id: user.id, email, name: user.name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const b = await body(c);
    const user = s.users.find((u) => u.email === String(b.email).toLowerCase() && u.password === b.password);
    if (!user) return fail(401, "unauthorized", "Invalid credentials");
    const token = `token-${++s.ids.token}`;
    s.sessions.set(token, user.id);
    return ok({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const token = c.request.headers.get("Authorization")?.replace(/^Bearer /, "");
    if (!token || !s.sessions.has(token)) {
      return fail(401, "unauthorized", "Authentication required");
    }
    s.sessions.delete(token);
    return noContent();
  }
  if (path === "/me" && method === "GET") {
    if (!c.user) fail(401, "unauthorized", "Authentication required");
    const u = s.users.find((x) => x.id === c.user)!;
    return ok({ id: u.id, email: u.email, name: u.name });
  }
  return null;
}
