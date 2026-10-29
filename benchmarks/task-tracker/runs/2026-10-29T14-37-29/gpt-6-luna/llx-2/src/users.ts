import type { State } from "./state";
import { body as _, error, invalid, json, auth, isError } from "./http";

type C = { request: Request; state: State; method: string; path: string; input: any };
export async function users(c: C): Promise<Response | undefined> {
  const { state, method, path, input, request } = c;
  if (path === "/users" && method === "POST") {
    if (!input) return error("validation", "Invalid JSON body", 400);
    const email = typeof input.email === "string" ? input.email.trim().toLowerCase() : "";
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)) return invalid("email");
    if (typeof input.password !== "string" || input.password.length < 8) return invalid("password");
    if (typeof input.name !== "string" || !input.name.trim() || input.name.trim().length > 100) return invalid("name");
    if (state.users.some((u) => u.email === email)) return error("conflict", "Email already exists", 409, "email");
    const user = { id: ++state.ids.user, email, name: input.name.trim(), password: input.password };
    state.users.push(user);
    return json({ id: user.id, email, name: user.name }, 201);
  }
  if (path === "/sessions" && method === "POST") {
    const email = typeof input?.email === "string" ? input.email.toLowerCase() : "";
    const user = state.users.find((u) => u.email === email && u.password === input?.password);
    if (!user) return error("unauthorized", "Invalid credentials", 401);
    const token = `session-${++state.ids.token}`;
    state.sessions.set(token, user.id);
    return json({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const value = auth(request, state);
    if (isError(value)) return value;
    const token = request.headers.get("Authorization")!.slice(7);
    state.sessions.delete(token);
    return new Response(null, { status: 204 });
  }
  if (path === "/me" && method === "GET") {
    const value = auth(request, state);
    return isError(value) ? value : json({ id: value.id, email: value.email, name: value.name });
  }
  if (["/users", "/sessions"].includes(path) && !["POST", "DELETE"].includes(method)) {
    return error("method_not_allowed", "Method not allowed", 405);
  }
  return undefined;
}
