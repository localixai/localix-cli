import { State, User, bad, conflict, error, json } from "./model";
export function userRoutes(s: State, method: string, path: string, body: any, uid?: number) {
  if (path === "/users" && method === "POST") {
    const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
    const name = typeof body.name === "string" ? body.name.trim() : "";
    if (!/^.+@.+\..+$/.test(email)) return bad("email");
    if (typeof body.password !== "string" || body.password.length < 8) return bad("password");
    if (!name || name.length > 100) return bad("name");
    if (s.users.some((u) => u.email === email)) return conflict("email");
    const user: User = { id: ++s.uid, email, password: body.password, name };
    s.users.push(user);
    return json({ id: user.id, email, name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const u = s.users.find((x) => x.email === String(body.email).toLowerCase() && x.password === body.password);
    if (!u) return error("unauthorized", "Invalid credentials", 401);
    const token = `token-${++s.tokenId}`;
    s.tokens.set(token, u.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const token = [...s.tokens].find(([, id]) => id === uid)?.[0];
    if (token) s.tokens.delete(token);
    return new Response(null, { status: 204 });
  }
  if (path === "/me" && method === "GET") {
    const u = s.users.find((x) => x.id === uid)!;
    return json({ id: u.id, email: u.email, name: u.name });
  }
}
