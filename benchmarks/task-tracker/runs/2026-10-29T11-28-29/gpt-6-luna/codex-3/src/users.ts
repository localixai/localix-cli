import type { State } from "./model";
import { auth, bad, bodyOf, conflict, fail, iso, json } from "./http";
export async function usersRoute(state: State, request: Request, path: string, method: string) {
  if (path === "/users" && method === "POST") {
    const b = await bodyOf(request);
    if (typeof b.email !== "string" || !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(b.email)) bad("email");
    if (typeof b.password !== "string" || b.password.length < 8) bad("password");
    if (typeof b.name !== "string" || !b.name.trim() || b.name.trim().length > 100) bad("name");
    const email = b.email.trim().toLowerCase();
    if (state.users.some((u) => u.email === email)) conflict("conflict", "email");
    const user = { id: state.nextUser++, email, password: b.password, name: b.name.trim() };
    state.users.push(user);
    return json({ id: user.id, email, name: user.name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const b = await bodyOf(request);
    const user = state.users.find((u) => u.email === String(b.email).toLowerCase() && u.password === b.password);
    if (!user) return fail(401, "unauthorized", "Unauthorized");
    const token = `token-${crypto.randomUUID()}`;
    state.tokens.set(token, user.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const user = auth(state, request, false);
    void user;
    state.tokens.delete((request.headers.get("Authorization") ?? "").slice(7));
    return new Response(null, { status: 204 });
  }
  if (path === "/me" && method === "GET") {
    const user = state.users.find((u) => u.id === auth(state, request));
    return json({ id: user!.id, email: user!.email, name: user!.name });
  }
  return null;
}
