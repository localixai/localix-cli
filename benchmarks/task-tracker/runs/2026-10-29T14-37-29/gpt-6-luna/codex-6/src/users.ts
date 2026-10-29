import type { Ctx, Result } from "./types";
import { fail, next, ok } from "./util";
export function userRoute(c: Ctx, path: string, method: string): Result | null {
  const s = c.state,
    b = c.body;
  if (path === "/users" && method === "POST") {
    if (typeof b?.email !== "string" || !/^\S+@\S+\.\S+$/.test(b.email))
      return fail(400, "validation", "invalid email", "email");
    if (typeof b.password !== "string" || b.password.length < 8)
      return fail(400, "validation", "password too short", "password");
    if (
      typeof b.name !== "string" ||
      !b.name.trim() ||
      b.name.trim().length > 100
    )
      return fail(400, "validation", "invalid name", "name");
    const email = b.email.trim().toLowerCase();
    if (s.users.some((u) => u.email === email))
      return fail(409, "conflict", "email already used", "email");
    const u = {
      id: next(c, "user"),
      email,
      password: b.password,
      name: b.name.trim(),
    };
    s.users.push(u);
    return ok({ id: u.id, email: u.email, name: u.name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const u = s.users.find(
      (x) =>
        x.email === String(b?.email).toLowerCase() &&
        x.password === b?.password,
    );
    if (!u) return fail(401, "unauthorized", "invalid credentials");
    const token = `token-${next(c, "token")}-${c.state.now()}`;
    s.sessions.set(token, u.id);
    return ok({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    s.sessions.delete(
      c.req.headers.get("Authorization")?.replace(/^Bearer /, "") || "",
    );
    return ok(undefined, 204);
  }
  if (path === "/me" && method === "GET")
    return c.user
      ? ok({ id: c.user.id, email: c.user.email, name: c.user.name })
      : fail(401, "unauthorized");
  return null;
}
