import type { Store } from "./model";
import { fail, ok, readBody, required } from "./http";

export async function users(req: Request, path: string, s: Store) {
  if (path === "/users" && req.method === "POST") {
    const b = await readBody(req);
    const email = required(b.email, "email", (v) => typeof v === "string" && /^[^@]+@[^@]+\.[^@]+$/.test(v));
    const password = required(b.password, "password", (v) => typeof v === "string" && v.length >= 8);
    const name = required(b.name, "name", (v) =>
      typeof v === "string" && v.trim().length > 0 && v.trim().length <= 100);
    if (s.users.some((u) => u.email === email.toLowerCase())) fail(409, "conflict", "email already exists", "email");
    const user = { id: ++s.uid, email: email.toLowerCase(), password, name: name.trim() };
    s.users.push(user);
    return ok({ id: user.id, email: user.email, name: user.name }, 201);
  }
  if (path === "/sessions" && req.method === "POST") {
    const b = await readBody(req);
    const u = s.users.find((x) => x.email === String(b.email).toLowerCase() && x.password === b.password);
    if (!u) fail(401, "unauthorized", "unauthorized");
    const token = `${u.id}-${s.now()}-${Math.random()}`;
    s.sessions.set(token, u.id);
    return ok({ token }, 201);
  }
  return null;
}
