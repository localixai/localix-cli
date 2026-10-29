import { json, fail, bad } from "./http";
export function usersRoute(c: any): Response {
  const { db, path, method, body: b, uid } = c;
  if (path === "/users" && method === "POST") {
    if (typeof b.email !== "string" || !/^[^@]+@[^@]+\.[^@]+$/.test(b.email)) return bad("email");
    if (typeof b.password !== "string" || b.password.length < 8) return bad("password");
    if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) return bad("name");
    const email = b.email.toLowerCase();
    if (db.users.some((u: any) => u.email === email)) return fail(409, "conflict", "Email exists", "email");
    const u = { id: ++db.counters.user, email, password: b.password, name: b.name.trim() };
    db.users.push(u);
    return json({ id: u.id, email, name: u.name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const u = db.users.find((x: any) => x.email === String(b.email).toLowerCase() && x.password === b.password);
    if (!u) return fail(401, "unauthorized", "Unauthorized");
    const token = `${u.id}-${crypto.randomUUID()}`; db.tokens.set(token, u.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const t = c.req.headers.get("Authorization")?.replace(/^Bearer /, "");
    if (!t || !db.tokens.has(t)) return fail(401, "unauthorized", "Unauthorized");
    db.tokens.delete(t); return json(null, 204);
  }
  if (path === "/me" && method === "GET") {
    const u = db.users.find((x: any) => x.id === uid);
    return u ? json({ id: u.id, email: u.email, name: u.name }) : fail(401, "unauthorized", "Unauthorized");
  }
  const r = fail(405, "method_not_allowed", "Method not allowed");
  if (path === "/users") r.headers.set("Allow", "POST");
  return r;
}
