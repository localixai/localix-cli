import { State, bodyObject, error, bad, iso } from "./model";

export function userRoute(s: State, method: string, path: string, user: number | undefined,
  token: string | undefined,
  data: Record<string, any> | null): Response | undefined {
  if (path === "/users" && method === "POST") {
    if (!data) return bad("body");
    const email = typeof data.email === "string" ? data.email.trim().toLowerCase() : "";
    const name = typeof data.name === "string" ? data.name.trim() : "";
    if (!email.includes("@") || !email.split("@")[1]?.includes(".")) return bad("email");
    if (typeof data.password !== "string" || data.password.length < 8) return bad("password");
    if (!name || name.length > 100) return bad("name");
    if (s.users.some(x => x.email === email)) return error("conflict", "Email is taken", 409, "email");
    const u = { id: ++s.ids.user, email, name, password: data.password };
    s.users.push(u);
    return Response.json({ id: u.id, email, name }, { status: 201 });
  }
  if (path === "/sessions" && method === "POST") {
    const email = typeof data?.email === "string" ? data.email.toLowerCase() : "";
    const u = s.users.find(x => x.email === email && x.password === data?.password);
    if (!u) return error("unauthorized", "Invalid credentials", 401);
    const token = `token-${++s.ids.token}`;
    s.tokens.set(token, u.id);
    return Response.json({ token }, { status: 201 });
  }
  if (path === "/sessions" && method === "DELETE") {
    if (token) s.tokens.delete(token);
    return new Response(null, { status: 204 });
  }
  if (path === "/me" && method === "GET") {
    const u = s.users.find(x => x.id === user);
    return Response.json({ id: u!.id, email: u!.email, name: u!.name });
  }
}
