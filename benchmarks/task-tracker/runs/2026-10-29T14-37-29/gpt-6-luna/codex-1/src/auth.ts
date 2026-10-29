import { bodyOf, fail, invalid, response } from "./errors";
import type { Context, User } from "./model";

export async function authRoute(ctx: Context, req: Request, path: string, method: string) {
  const { state } = ctx;
  if (path === "/users" && method === "POST") {
    const b = await bodyOf(req);
    if (typeof b.email !== "string" || !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(b.email)) invalid("email");
    if (typeof b.password !== "string" || b.password.length < 8) invalid("password");
    if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) invalid("name");
    const email = b.email.toLowerCase();
    if (state.users.some((u) => u.email === email)) fail(409, "conflict", "Email is already taken", "email");
    const user = { id: state.nextUser++, email, password: b.password, name: b.name.trim() };
    state.users.push(user);
    return response(publicUser(user), 201);
  }
  if (path === "/sessions" && method === "POST") {
    const b = await bodyOf(req);
    const user = state.users.find((u) => u.email === String(b.email).toLowerCase() && u.password === b.password);
    if (!user) fail(401, "unauthorized", "Invalid email or password");
    const token = `${user.id}-${crypto.randomUUID()}`;
    state.sessions.set(token, user.id);
    return response({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const token = bearer(req);
    if (!token || !state.sessions.has(token)) fail(401, "unauthorized", "Authentication required");
    state.sessions.delete(token);
    return response(null, 204);
  }
  if (path === "/me" && method === "GET") {
    return response(publicUser(requireUser(ctx, req)));
  }
  return null;
}
export function bearer(req: Request): string | undefined {
  const value = req.headers.get("Authorization");
  return value?.startsWith("Bearer ") ? value.slice(7) : undefined;
}
export function requireUser(ctx: Context, req: Request): User {
  const id = ctx.state.sessions.get(bearer(req) ?? "");
  const user = ctx.state.users.find((item) => item.id === id);
  if (!user) fail(401, "unauthorized", "Authentication required");
  return user;
}
export const publicUser = ({ id, email, name }: User) => ({ id, email, name });
