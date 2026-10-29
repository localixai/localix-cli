import type { State } from "./model";
import { publicUser } from "./model";
import { check, fail } from "./util";

export async function identity(
  s: State,
  method: string,
  path: string,
  body: Record<string, any>,
  uid: number | undefined,
) {
  if (path === "/users" && method === "POST") {
    check(typeof body.email === "string" && /^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(body.email), "email");
    check(typeof body.password === "string" && body.password.length >= 8, "password");
    check(typeof body.name === "string" && body.name.trim().length > 0 && body.name.trim().length <= 100, "name");
    const email = body.email.toLowerCase();
    if (s.users.some((u) => u.email === email)) fail(409, "conflict", "email is already taken", "email");
    const user = {
      id: s.users.length + 1,
      email,
      password: body.password,
      name: body.name.trim(),
    };
    s.users.push(user);
    return { status: 201, body: publicUser(user) };
  }
  if (path === "/sessions" && method === "POST") {
    const user = s.users.find((u) => u.email === String(body.email).toLowerCase() && u.password === body.password);
    if (!user) fail(401, "unauthorized", "email or password is incorrect");
    const token = `token-${crypto.randomUUID()}`;
    s.tokens.set(token, user.id);
    return { status: 201, body: { token } };
  }
  if (path === "/sessions" && method === "DELETE") {
    const token = [...s.tokens].find(([, id]) => id === uid)?.[0];
    if (token) s.tokens.delete(token);
    return { status: 204, body: null };
  }
  if (path === "/me" && method === "GET") {
    const user = s.users.find((u) => u.id === uid)!;
    return { status: 200, body: publicUser(user) };
  }
}
