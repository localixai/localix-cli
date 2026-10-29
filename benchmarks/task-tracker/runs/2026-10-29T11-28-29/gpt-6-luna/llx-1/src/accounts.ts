import { auth, fail, obj, valid } from "./common";
import type { State } from "./model";
export async function account(s: State, req: Request, path: string) {
  if (path === "/users" && req.method === "POST") {
    const b = obj(await req.json().catch(() => fail(400, "validation")));
    valid(typeof b.email === "string" && /^[^@]+@[^@]+\.[^@]+$/.test(b.email), "email");
    valid(typeof b.password === "string" && b.password.length >= 8, "password");
    valid(typeof b.name === "string" && b.name.trim().length > 0 && b.name.trim().length <= 100, "name");
    const email = b.email.toLowerCase();
    if (s.users.some(u => u.email === email)) fail(409, "conflict", "email");
    const u = { id: s.nextUser++, email, name: b.name.trim(), password: b.password };
    s.users.push(u);
    return [201, { id: u.id, email, name: u.name }];
  }
  if (path === "/sessions" && req.method === "POST") {
    const b = obj(await req.json().catch(() => fail(400, "validation")));
    const u = s.users.find(x => x.email === String(b.email).toLowerCase() && x.password === b.password);
    if (!u) fail(401, "unauthorized");
    const token = `session-${s.nextToken++}`;
    s.tokens.set(token, u!.id);
    return [201, { token }];
  }
  if (path === "/sessions" && req.method === "DELETE") {
    const id = auth(s, req);
    const token = req.headers.get("Authorization")!.slice(7);
    void id; s.tokens.delete(token); return [204, null];
  }
  if (path === "/me" && req.method === "GET") {
    const u = s.users.find(x => x.id === auth(s, req))!;
    return [200, { id: u.id, email: u.email, name: u.name }];
  }
  return null;
}
