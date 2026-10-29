import type { State } from "./model";
import { cleanUser } from "./model";
import { ApiError, auth, body, respond } from "./http";
export async function account(
  req: Request,
  path: string,
  s: State,
): Promise<Response | null> {
  const method = req.method;
  if (path === "/users" && method === "POST") {
    const b = await body(req);
    const email =
      typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email))
      throw new ApiError(400, "validation", "email is invalid", "email");
    if (typeof b.password !== "string" || b.password.length < 8)
      throw new ApiError(400, "validation", "password is invalid", "password");
    const name = typeof b.name === "string" ? b.name.trim() : "";
    if (!name || name.length > 100)
      throw new ApiError(400, "validation", "name is invalid", "name");
    if (s.users.some((u) => u.email === email))
      throw new ApiError(409, "conflict", "email already taken", "email");
    const u = { id: s.users.length + 1, email, password: b.password, name };
    s.users.push(u);
    return respond(cleanUser(u), 201);
  }
  if (path === "/sessions" && method === "POST") {
    const b = await body(req);
    const email = typeof b.email === "string" ? b.email.toLowerCase() : "";
    const u = s.users.find(
      (x) => x.email === email && x.password === b.password,
    );
    if (!u)
      throw new ApiError(401, "unauthorized", "email or password is incorrect");
    const token = crypto.randomUUID();
    s.sessions.set(token, u.id);
    return respond({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const uid = auth(req, s);
    for (const [token, id] of s.sessions)
      if (id === uid && token === req.headers.get("Authorization")?.slice(7))
        s.sessions.delete(token);
    return respond(null, 204);
  }
  if (path === "/me" && method === "GET") {
    const uid = auth(req, s);
    const u = s.users.find((x) => x.id === uid)!;
    return respond(cleanUser(u));
  }
  return null;
}
