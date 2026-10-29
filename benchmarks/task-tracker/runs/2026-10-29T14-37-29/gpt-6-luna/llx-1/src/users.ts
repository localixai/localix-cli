import { body, err, json, type Ctx } from "./http";
export async function users(c: Ctx, path: string) {
  if (path === "/users" && c.req.method === "POST") {
    const b = await body(c); if (!b) return err("validation", "Invalid JSON");
    for (const f of ["email", "password", "name"]) {
      if (typeof b[f] !== "string" || !b[f].trim() || (f === "password" && b[f].length < 8) ||
        (f === "name" && b[f].trim().length > 100) ||
        (f === "email" && !/^[^@]+@[^@]+\.[^@]+$/.test(b[f]))) return err("validation", `Invalid ${f}`, f);
    }
    const email = b.email.toLowerCase();
    if (c.s.users.some(u => u.email === email)) return err("conflict", "Email already exists", "email", 409);
    const u = { id: c.s.users.length + 1, email, password: b.password, name: b.name.trim() };
    c.s.users.push(u); return json({ id: u.id, email, name: u.name }, 201);
  }
  if (path === "/sessions" && c.req.method === "POST") {
    const b = await body(c), u = c.s.users.find(x => x.email === String(b?.email).toLowerCase());
    if (!u || u.password !== b?.password) return err("unauthorized", "Unauthorized", undefined, 401);
    const token = crypto.randomUUID(); c.s.tokens.set(token, u.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && c.req.method === "DELETE") {
    const token = c.req.headers.get("Authorization")?.replace(/^Bearer /, "");
    c.s.tokens.delete(token || ""); return new Response(null, { status: 204 });
  }
  if (path === "/me" && c.req.method === "GET") {
    const u = c.s.users.find(x => x.id === c.user)!;
    return json({ id: u.id, email: u.email, name: u.name });
  }
  return null;
}
