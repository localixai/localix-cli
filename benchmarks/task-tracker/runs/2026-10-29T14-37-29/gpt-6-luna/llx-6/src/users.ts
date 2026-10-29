import { error, ok, type State } from "./model";
export function users(s: State, method: string, path: string, body: any, token: string | null) {
  if (path === "/users" && method === "POST") {
    if (!body || typeof body !== "object" || Array.isArray(body)) return error("validation", "Invalid body", 400);
    const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
    const name = typeof body.name === "string" ? body.name.trim() : "";
    if (!/^[^@]+@[^@]+\.[^@]+$/.test(email))
      return error("validation", "Invalid email", 400, "email");
    if (typeof body.password !== "string" || body.password.length < 8)
      return error("validation", "Invalid password", 400, "password");
    if (!name || name.length > 100) return error("validation", "Invalid name", 400, "name");
    if (s.users.some(u => u.email === email)) return error("conflict", "Email is taken", 409, "email");
    const u = { id: ++s.ids.user, email, password: body.password, name };
    s.users.push(u); return ok({ id: u.id, email, name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const u = s.users.find(x => x.email === String(body?.email).toLowerCase() && x.password === body?.password);
    if (!u) return error("unauthorized", "Invalid credentials", 401);
    const t = `token-${u.id}-${s.now()}-${Math.random()}`;
    s.sessions.set(t, u.id); return ok({ token: t }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    if (!token || !s.sessions.has(token)) return error("unauthorized", "Unauthorized", 401);
    s.sessions.delete(token); return ok(null, 204);
  }
  if (path === "/me" && method === "GET") {
    const u = token ? s.users.find(x => x.id === s.sessions.get(token)) : undefined;
    if (!u) return error("unauthorized", "Unauthorized", 401);
    return ok({ id: u.id, email: u.email, name: u.name });
  }
  return null;
}
