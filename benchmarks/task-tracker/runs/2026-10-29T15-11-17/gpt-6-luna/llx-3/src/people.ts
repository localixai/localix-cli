import type { State } from "./model";
import { fail, bad, gone } from "./util";

export function people(s: State, method: string, path: string, body: any, user: number | null) {
  if (method === "POST" && path === "/users") {
    const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
    const name = typeof body.name === "string" ? body.name.trim() : "";
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)) return bad("email");
    if (typeof body.password !== "string" || body.password.length < 8) return bad("password");
    if (!name || name.length > 100) return bad("name");
    if (s.users.some(u => u.email === email)) return fail("conflict", "Email already exists", 409, "email");
    const u = { id: s.next.user++, email, password: body.password, name };
    s.users.push(u);
    return Response.json({ id: u.id, email, name }, { status: 201 });
  }
  if (method === "POST" && path === "/sessions") {
    const email = String(body.email ?? "").toLowerCase();
    const u = s.users.find(x => x.email === email && x.password === body.password);
    if (!u) return fail("unauthorized", "Unauthorized", 401);
    const token = `token-${u.id}-${crypto.randomUUID()}`;
    s.sessions.set(token, u.id);
    return Response.json({ token }, { status: 201 });
  }
  if (method === "GET" && path === "/me") {
    const u = s.users.find(x => x.id === user);
    return u ? Response.json({ id: u.id, email: u.email, name: u.name }) :
      fail("unauthorized", "Unauthorized", 401);
  }
  if (method === "DELETE" && path === "/sessions") {
    return new Response(null, { status: 204 });
  }
  return null;
}

export function projects(s: State, method: string, path: string, body: any, user: number, now: string) {
  if (path === "/projects" && method === "POST") {
    const name = typeof body.name === "string" ? body.name.trim() : "";
    if (!name || name.length > 100) return bad("name");
    if (typeof body.key !== "string" || !/^[A-Z]{2,6}$/.test(body.key)) return bad("key");
    if (s.projects.some(p => p.key === body.key)) return fail("conflict", "Key exists", 409, "key");
    const p = { id: s.next.project++, name, key: body.key, ownerId: user, createdAt: now };
    s.projects.push(p); s.members.set(p.id, new Map([[user, "owner"]]));
    return Response.json(p, { status: 201 });
  }
  if (path === "/projects" && method === "GET") {
    return Response.json(s.projects.filter(p => s.members.get(p.id)?.has(user)));
  }
  return null;
}
