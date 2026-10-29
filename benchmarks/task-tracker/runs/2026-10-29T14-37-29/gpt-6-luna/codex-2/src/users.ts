import { Context, fail, json, nextId, User } from "./model";
export function userRoutes(
  c: Context,
  path: string,
  method: string,
  body: any,
) {
  const s = c.state;
  if (path === "/users" && method === "POST") {
    const email =
      typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
    const password = body.password,
      name = typeof body.name === "string" ? body.name.trim() : "";
    if (!/^.+@.+\..+$/.test(email))
      return fail("validation", "invalid email", 400, "email");
    if (typeof password !== "string" || password.length < 8)
      return fail("validation", "password is too short", 400, "password");
    if (!name || name.length > 100)
      return fail("validation", "invalid name", 400, "name");
    if (s.users.some((u) => u.email === email))
      return fail("conflict", "email already exists", 409, "email");
    const u: User = { id: nextId(s, "user"), email, password, name };
    s.users.push(u);
    return json({ id: u.id, email, name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const email =
      typeof body.email === "string" ? body.email.toLowerCase() : "";
    const user = s.users.find(
      (u) => u.email === email && u.password === body.password,
    );
    if (!user) return fail("unauthorized", "invalid credentials", 401);
    const token = crypto.randomUUID();
    s.sessions.set(token, user.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const auth = c.request.headers.get("Authorization") ?? "";
    if (!auth.startsWith("Bearer "))
      return fail("unauthorized", "authentication required", 401);
    s.sessions.delete(auth.slice(7));
    return { status: 204 };
  }
  if (path === "/me" && method === "GET") {
    const u = s.users.find((x) => x.id === c.userId)!;
    return json({ id: u.id, email: u.email, name: u.name });
  }
}
