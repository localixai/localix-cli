import { body, fail, json, person, validate, type State } from "./model";
export async function users(
  req: Request,
  path: string,
  s: State,
  u: number | undefined,
) {
  if (path === "/users" && req.method === "POST") {
    const b = await body(req),
      email = typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
    validate(/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email), "email");
    validate(
      typeof b.password === "string" && b.password.length >= 8,
      "password",
    );
    const name = typeof b.name === "string" ? b.name.trim() : "";
    validate(name.length > 0 && name.length <= 100, "name");
    if (s.users.some((x) => x.email === email))
      fail(409, "conflict", "email already used", "email");
    const user = { id: ++s.counters.user, email, password: b.password, name };
    s.users.push(user);
    return json(person(user), 201);
  }
  if (path === "/sessions" && req.method === "POST") {
    const b = await body(req),
      email = String(b.email || "").toLowerCase(),
      user = s.users.find(
        (x) => x.email === email && x.password === b.password,
      );
    if (!user) fail(401, "unauthorized");
    const token = `session-${user.id}-${s.now()}-${Math.random()}`;
    s.sessions.set(token, user.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && req.method === "DELETE") {
    if (!u) fail(401, "unauthorized");
    const token = req.headers.get("Authorization")?.slice(7) || "";
    s.sessions.delete(token);
    return new Response(null, { status: 204 });
  }
  if (path === "/me" && req.method === "GET") {
    if (!u) fail(401, "unauthorized");
    return json(person(s.users.find((x) => x.id === u)!));
  }
  return undefined;
}
