import type { State } from "./model";
import { auth, fail, obj, response, validation } from "./http";

export async function users(method: string, path: string[], req: Request, s: State): Promise<Response> {
  if (path[0] === "users") {
    if (method !== "POST") return response(null, 405, { Allow: "POST" });
    const b = await body(req);
    if (typeof b.email !== "string" || !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(b.email)) validation("email");
    if (typeof b.password !== "string" || b.password.length < 8) validation("password");
    if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) validation("name");
    const email = b.email.toLowerCase();
    if (s.users.some((u) => u.email === email)) fail(409, "conflict", "Email already exists", "email");
    const u = { id: ++s.ids.user, email, name: b.name.trim(), password: b.password };
    s.users.push(u);
    return response({ id: u.id, email, name: u.name }, 201);
  }
  if (path[0] === "sessions") {
    if (method === "POST") {
      const b = await body(req);
      const u = s.users.find((x) => x.email === String(b.email).toLowerCase());
      if (!u || u.password !== b.password) fail(401, "unauthorized", "Invalid email or password");
      const token = `${u.id}-${crypto.randomUUID()}`;
      s.sessions.set(token, u.id);
      return response({ token }, 201);
    }
    if (method === "DELETE") {
      const id = auth(req, s);
      const token = req.headers.get("Authorization")!.slice(7);
      void id;
      s.sessions.delete(token);
      return response(null, 204);
    }
    return response(null, 405, { Allow: "POST, DELETE" });
  }
  if (path[0] === "me") {
    if (method !== "GET") return response(null, 405, { Allow: "GET" });
    const id = auth(req, s);
    const u = s.users.find((x) => x.id === id)!;
    return response({ id, email: u.email, name: u.name });
  }
  return response({ error: { code: "not_found", message: "Not found" } }, 404);
}
export async function body(req: Request): Promise<Record<string, any>> {
  try { return obj(await req.json()); } catch (e) {
    if (e instanceof SyntaxError) fail(400, "validation", "Invalid JSON");
    if (e instanceof TypeError) validation("body");
    throw e;
  }
}
