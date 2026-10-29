import { err, iso, ok, userView, type State } from "./model";
export async function usersRoute(
  s: State,
  method: string,
  path: string,
  body: any,
  who: number | undefined,
): Promise<Response | null> {
  if (path === "/users" && method === "POST") {
    const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
    const name = typeof body.name === "string" ? body.name.trim() : "";
    if (!email.includes("@") || !email.split("@")[1]?.includes("."))
      return err("validation", "email is invalid", 400, "email");
    if (typeof body.password !== "string" || body.password.length < 8)
      return err("validation", "password is too short", 400, "password");
    if (!name || name.length > 100) return err("validation", "name is invalid", 400, "name");
    if (s.users.some((u) => u.email === email))
      return err("conflict", "email already exists", 409, "email");
    const u = { id: s.users.length + 1, email, name, password: body.password };
    s.users.push(u);
    return ok(userView(u), 201);
  }
  if (path === "/sessions" && method === "POST") {
    const u = s.users.find(
      (x) => x.email === String(body.email).toLowerCase() && x.password === body.password,
    );
    if (!u) return err("unauthorized", "invalid email or password", 401);
    const token = `token-${u.id}-${s.now()}-${Math.random()}`;
    s.sessions.set(token, u.id);
    return ok({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    if (!who) return err("unauthorized", "unauthorized", 401);
    const token = [...s.sessions].find(([, id]) => id === who);
    if (token) s.sessions.delete(token[0]);
    return new Response(null, { status: 204 });
  }
  if (path === "/me" && method === "GET") {
    const u = s.users.find((x) => x.id === who);
    return u ? ok(userView(u)) : err("unauthorized", "unauthorized", 401);
  }
  return null;
}
