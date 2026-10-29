import type { State } from "./state";
import { auth, bad, body, err, json, userView } from "./util";

export async function usersRoute(req: Request, s: State, u: any, path: string) {
  if (path === "/users" && req.method === "POST") {
    const b = await body(req);
    if (!b) return err(400, "validation", "Invalid JSON");
    const email = typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
    const name = typeof b.name === "string" ? b.name.trim() : "";
    if (!/^[^@]+@[^@]+\.[^@]+$/.test(email)) return bad("email");
    if (typeof b.password !== "string" || b.password.length < 8) return bad("password");
    if (!name || name.length > 100) return bad("name");
    if (s.users.some(x => x.email === email)) return err(409, "conflict", "Email already taken", "email");
    const user = { id: s.users.length + 1, email, name, password: b.password };
    s.users.push(user);
    return json(userView(user), 201);
  }
  if (path === "/sessions" && req.method === "POST") {
    const b = await body(req);
    const user = b && s.users.find(x => x.email === String(b.email).toLowerCase() && x.password === b.password);
    if (!user) return err(401, "unauthorized", "Invalid credentials");
    const token = crypto.randomUUID();
    s.sessions.set(token, user.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && req.method === "DELETE") {
    const m = /^Bearer (.+)$/.exec(req.headers.get("Authorization") ?? "");
    if (!m || !s.sessions.has(m[1]!)) return err(401, "unauthorized", "Authentication required");
    s.sessions.delete(m[1]!);
    return new Response(null, { status: 204 });
  }
  if (path === "/me" && req.method === "GET") return json(userView(u));
  return null;
}
