import type { State } from "./model";
import { auth, bad, body, fail, json, empty } from "./http";

export async function accounts(req: Request, path: string[], s: State): Promise<Response | undefined> {
  const method = req.method;
  if (path[0] === "users" && path.length === 1) {
    if (method !== "POST") return undefined;
    const b = await body(req);
    if (typeof b.email !== "string" || !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(b.email)) bad("email");
    if (typeof b.password !== "string" || b.password.length < 8) bad("password");
    if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) bad("name");
    const email = b.email.trim().toLowerCase();
    if (s.users.some((u) => u.email === email)) fail(409, "conflict", "Email already exists", "email");
    const user = { id: ++s.ids.user, email, password: b.password, name: b.name.trim() };
    s.users.push(user);
    return json({ id: user.id, email, name: user.name }, 201);
  }
  if (path[0] === "sessions" && path.length === 1) {
    if (method === "POST") {
      const b = await body(req);
      const user = s.users.find((u) => u.email === String(b.email).toLowerCase() && u.password === b.password);
      if (!user) fail(401, "unauthorized", "Invalid email or password");
      const token = `token-${++s.ids.token}`;
      s.tokens.set(token, user.id);
      return json({ token }, 201);
    }
    if (method === "DELETE") {
      const token = /^Bearer (.+)$/.exec(req.headers.get("Authorization") ?? "")?.[1] ?? "";
      s.tokens.delete(token);
      return empty();
    }
    return undefined;
  }
  if (path[0] === "me" && path.length === 1 && method === "GET") {
    const id = auth(req, s);
    const u = s.users.find((x) => x.id === id)!;
    return json({ id, email: u.email, name: u.name });
  }
  return undefined;
}
