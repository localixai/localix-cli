import { Ctx, Result, publicUser, validation } from "./routes";
import { fail } from "./http";
export function accounts(c: Ctx): Result {
  const { db, body: b } = c;
  if (c.method === "POST" && c.path === "/users") {
    if (typeof b.email !== "string" || !/^[^@]+@[^@]+\.[^@]+$/.test(b.email)) validation("email");
    if (typeof b.password !== "string" || b.password.length < 8) validation("password");
    if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) validation("name");
    const email = b.email.trim().toLowerCase();
    if (db.users.some(u => u.email === email)) fail(409, "conflict", "Email already exists", "email");
    const u = { id: db.users.length + 1, email, name: b.name.trim(), password: b.password };
    db.users.push(u); return { status: 201, body: publicUser(u) };
  }
  if (c.path === "/sessions" && c.method !== "POST") {
    if (c.method === "DELETE") {
      const uid = c.token ? db.tokens.get(c.token) : undefined;
      if (uid === undefined) fail(401, "unauthorized", "Authentication required");
      db.tokens.delete(c.token); return { status: 204 };
    }
    fail(405, "method_not_allowed", "Method not allowed");
  }
  const email = typeof b.email === "string" ? b.email.toLowerCase() : "";
  const u = db.users.find(x => x.email === email && x.password === b.password);
  if (!u) fail(401, "unauthorized", "Invalid credentials");
  const token = `${u.id}-${crypto.randomUUID()}`;
  db.tokens.set(token, u.id);
  return { status: 201, body: { token } };
}
