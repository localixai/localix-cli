import { State, User, err, ok } from "./model";
export async function authRoutes(
  s: State,
  r: Request,
  path: string,
  method: string,
  user: number | undefined,
): Promise<Response | undefined> {
  let b: any;
  const body = async () => {
    try {
      const x = await r.json();
      return x && typeof x === "object" && !Array.isArray(x) ? x : null;
    } catch {
      return null;
    }
  };
  if (path === "/users" && method === "POST") {
    b = await body();
    if (!b) return err("validation", "Expected an object");
    if (typeof b.email !== "string" || !/^.+@.+\..+$/.test(b.email))
      return err("validation", "Invalid email", "email");
    if (typeof b.password !== "string" || b.password.length < 8)
      return err("validation", "Invalid password", "password");
    if (
      typeof b.name !== "string" ||
      !b.name.trim() ||
      b.name.trim().length > 100
    )
      return err("validation", "Invalid name", "name");
    const email = b.email.trim().toLowerCase();
    if (s.users.some((x) => x.email === email))
      return err("conflict", "Email already exists", "email", 409);
    const u = {
      id: ++s.ids.user,
      email,
      password: b.password,
      name: b.name.trim(),
    };
    s.users.push(u);
    return ok({ id: u.id, email: u.email, name: u.name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    b = await body();
    if (!b) return err("validation", "Expected an object");
    const u = s.users.find(
      (x) =>
        x.email === String(b.email).toLowerCase() && x.password === b.password,
    );
    if (!u) return err("unauthorized", "Invalid credentials", undefined, 401);
    const token = `token-${crypto.randomUUID()}`;
    s.sessions.set(token, u.id);
    return ok({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    if (user === undefined)
      return err("unauthorized", "Unauthorized", undefined, 401);
    const token = r.headers.get("Authorization")?.replace(/^Bearer /, "");
    if (token) s.sessions.delete(token);
    return new Response(null, { status: 204 });
  }
  if (path === "/me" && method === "GET") {
    const u = s.users.find((x) => x.id === user);
    return u
      ? ok(publicUser(u))
      : err("unauthorized", "Unauthorized", undefined, 401);
  }
  return undefined;
}
export function publicUser(u: User) {
  return { id: u.id, email: u.email, name: u.name };
}
