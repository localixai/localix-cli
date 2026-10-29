import { fail, json, safe, stamp, type State } from "./model";
export async function userRoute(
  s: State,
  method: string,
  path: string,
  body: any,
) {
  if (path === "/users" && method === "POST") {
    const email =
      typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
    const name = typeof body.name === "string" ? body.name.trim() : "";
    if (!email.includes("@") || !email.split("@")[1]?.includes("."))
      return fail(400, "validation", "Invalid email", "email");
    if (typeof body.password !== "string" || body.password.length < 8)
      return fail(400, "validation", "Invalid password", "password");
    if (!name || name.length > 100)
      return fail(400, "validation", "Invalid name", "name");
    if (s.users.some((u) => u.email === email))
      return fail(409, "conflict", "Email already exists", "email");
    const u = { id: ++s.ids.user, email, name, password: body.password };
    s.users.push(u);
    return json(safe(u), 201);
  }
  if (path === "/sessions" && method === "POST") {
    const u = s.users.find(
      (x) =>
        x.email === String(body.email ?? "").toLowerCase() &&
        x.password === body.password,
    );
    if (!u) return fail(401, "unauthorized", "Invalid credentials");
    const token = crypto.randomUUID();
    s.sessions.set(token, u.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE")
    return new Response(null, { status: 204 });
  return undefined;
}
export function authenticate(s: State, req: Request) {
  const h = req.headers.get("Authorization") ?? "";
  if (!h.startsWith("Bearer ")) return undefined;
  const token = h.slice(7);
  const id = s.sessions.get(token);
  return id === undefined ? undefined : { id, token };
}
