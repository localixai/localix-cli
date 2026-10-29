import type { State } from "./model";
import { fail, need, obj } from "./util";

export function userRoutes(s: State, method: string, path: string, body: any, token: string | undefined) {
  if (path === "/users" && method === "POST") {
    const b = obj(body);
    const email = typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
    need(/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email), "email");
    need(typeof b.password === "string" && b.password.length >= 8, "password");
    const name = typeof b.name === "string" ? b.name.trim() : "";
    need(name.length > 0 && name.length <= 100, "name");
    if (s.users.some(u => u.email === email)) fail(409, "conflict", "Email already exists", "email");
    const u = { id: ++s.ids.user, email, name, password: b.password };
    s.users.push(u);
    return [201, { id: u.id, email, name }];
  }
  if (path === "/sessions" && method === "POST") {
    const b = obj(body);
    const email = typeof b.email === "string" ? b.email.toLowerCase() : "";
    const u = s.users.find(x => x.email === email && x.password === b.password);
    if (!u) fail(401, "unauthorized", "Invalid email or password");
    const id = crypto.randomUUID();
    s.sessions.set(id, u.id);
    return [201, { token: id }];
  }
  if (path === "/sessions" && method === "DELETE") {
    if (!token || !s.sessions.has(token)) fail(401, "unauthorized", "Authentication required");
    s.sessions.delete(token);
    return [204, null];
  }
  if (path === "/me" && method === "GET") {
    const u = s.users.find(x => x.id === auth(s, token));
    return [200, { id: u!.id, email: u!.email, name: u!.name }];
  }
  return null;
}
export function auth(s: State, token?: string): number {
  const id = token ? s.sessions.get(token) : undefined;
  if (!id) fail(401, "unauthorized", "Authentication required");
  return id;
}
