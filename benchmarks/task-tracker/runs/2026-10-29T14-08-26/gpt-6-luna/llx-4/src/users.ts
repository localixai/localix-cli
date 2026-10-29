import type { State } from "./types";
import { fail, valid } from "./util";
const json = (x: unknown, status = 200) => Response.json(x, { status });
const noContent = () => new Response(null, { status: 204 });
export function users(s: State, method: string, path: string, b: any, uid: number, token: string): Response {
  if (path === "/users" && method === "POST") {
    const email = typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
    const name = typeof b.name === "string" ? b.name.trim() : "";
    valid(/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email), "email");
    valid(typeof b.password === "string" && b.password.length >= 8, "password");
    valid(name.length > 0 && name.length <= 100, "name");
    if (s.users.some((u) => u.email === email)) fail(409, "conflict", "Email taken", "email");
    const u = { id: ++s.ids.user, email, password: b.password, name };
    s.users.push(u); return json({ id: u.id, email, name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const u = s.users.find((x) => x.email === String(b.email).toLowerCase() && x.password === b.password);
    if (!u) return fail(401, "unauthorized");
    const t = `${u.id}-${crypto.randomUUID()}`; s.tokens.set(t, u.id); return json({ token: t }, 201);
  }
  if (path === "/sessions" && method === "DELETE") { s.tokens.delete(token); return noContent(); }
  if (path === "/me") {
    const u = s.users.find((x) => x.id === uid)!;
    return json({ id: u.id, email: u.email, name: u.name });
  }
  return fail(405, "method_not_allowed");
}
