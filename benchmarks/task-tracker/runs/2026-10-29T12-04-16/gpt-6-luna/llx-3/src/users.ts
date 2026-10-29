import type { State } from "./model";
import { answer, fail } from "./common";

export function userRoutes(s: State, method: string, path: string, b: any, req: Request) {
  if (path === "/users" && method === "POST") {
    if (!b) return fail(400, "validation");
    const email = typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
    const name = typeof b.name === "string" ? b.name.trim() : "";
    for (const [ok, f] of [[/^[^@]+@[^@]+\.[^@]+$/.test(email), "email"],
      [typeof b.password === "string" && b.password.length >= 8, "password"],
      [!!name && name.length <= 100, "name"]] as [boolean, string][])
      if (!ok) return fail(400, "validation", "Invalid " + f, f);
    if (s.users.some(u => u.email === email)) return fail(409, "conflict", "Email taken", "email");
    const u = { id: s.users.length + 1, email, name, password: b.password };
    s.users.push(u);
    return answer({ id: u.id, email, name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const email = String(b?.email ?? "").toLowerCase();
    const u = s.users.find(x => x.email === email && x.password === b?.password);
    if (!u) return fail(401, "unauthorized");
    const token = crypto.randomUUID();
    s.tokens.set(token, u.id);
    return answer({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const token = req.headers.get("Authorization")?.replace(/^Bearer /, "");
    if (!token || !s.tokens.has(token)) return fail(401, "unauthorized");
    s.tokens.delete(token);
    return answer(null, 204);
  }
  return null;
}
