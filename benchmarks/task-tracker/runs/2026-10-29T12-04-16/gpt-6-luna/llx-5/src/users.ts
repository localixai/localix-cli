import { auth, json } from "./response";
import { validate, type Ctx, type User } from "./model";
export function userRoute(c: Ctx, path: string) {
  const { s, body } = c;
  if (path === "/users" && c.req.method === "POST") {
    const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
    const name = typeof body.name === "string" ? body.name.trim() : "";
    validate(/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email), "email");
    validate(typeof body.password === "string" && body.password.length >= 8, "password");
    validate(name.length > 0 && name.length <= 100, "name");
    if (s.users.some(u => u.email === email))
      throw Object.assign(Error("Email already taken"), {
        status: 409, code: "conflict", field: "email",
      });
    const u: User = { id: (s.ids.user = s.ids.user! + 1), email, name, password: body.password };
    s.users.push(u);
    return json({ id: u.id, email, name }, 201);
  }
  if (path === "/sessions" && c.req.method === "POST") {
    const u = s.users.find(x => x.email === String(body.email).toLowerCase() && x.password === body.password);
    if (!u) throw Object.assign(Error("Unauthorized"), { status: 401, code: "unauthorized" });
    const token = `${u.id}-${s.now()}-${Math.random()}`; s.tokens.set(token, u.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && c.req.method === "DELETE") {
    const t = auth(c); s.tokens.delete(t); return new Response(null, { status: 204 });
  }
  if (path === "/me" && c.req.method === "GET") {
    auth(c); const { id, email, name } = c.user!; return json({ id, email, name });
  }
}
