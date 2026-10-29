import { body, err, json, validation, type Ctx } from "./model";
export async function users(c: Ctx, path: string, method: string) {
  if (path === "/users" && method === "POST") {
    const b = await body(c);
    if (!b) return validation();
    if (
      typeof b.email !== "string" ||
      !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(b.email.trim())
    )
      return validation("email");
    if (typeof b.password !== "string" || b.password.length < 8)
      return validation("password");
    if (
      typeof b.name !== "string" ||
      !b.name.trim() ||
      b.name.trim().length > 100
    )
      return validation("name");
    const email = b.email.trim().toLowerCase();
    if (c.db.users.some((u) => u.email === email))
      return err("conflict", 409, "email");
    const u = {
      id: c.db.users.length + 1,
      email,
      name: b.name.trim(),
      password: b.password,
    };
    c.db.users.push(u);
    return json({ id: u.id, email, name: u.name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const b = await body(c);
    if (!b) return validation();
    const u = c.db.users.find(
      (x) =>
        x.email === String(b.email).toLowerCase() && x.password === b.password,
    );
    if (!u) return err("unauthorized", 401);
    const token = `session-${c.now()}-${Math.random()}-${c.db.sessions.size}`;
    c.db.sessions.set(token, u.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const token = c.req.headers
      .get("Authorization")
      ?.match(/^Bearer (.+)$/)?.[1];
    if (!c.user) return err("unauthorized", 401);
    if (token) c.db.sessions.delete(token);
    return new Response(null, { status: 204 });
  }
  if (path === "/me" && method === "GET")
    return json({ id: c.user!.id, email: c.user!.email, name: c.user!.name });
  return null;
}
