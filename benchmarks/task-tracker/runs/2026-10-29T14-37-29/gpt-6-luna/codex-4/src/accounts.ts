import type { Ctx } from './model';
import { next } from './model';
import { body, fail, json, need, user } from './http';
export async function accountRoute(c: Ctx, req: Request, path: string) {
  const m = req.method;
  if (path === '/users' && m === 'POST') {
    const b = await body(req); const email = typeof b.email === 'string' ? b.email.trim().toLowerCase() : '';
    need(/^\S+@\S+\.\S+$/.test(email), 'email');
    need(typeof b.password === 'string' && b.password.length >= 8, 'password');
    need(typeof b.name === 'string' && b.name.trim().length > 0 && b.name.trim().length <= 100, 'name');
    if (c.s.users.some(x => x.email === email)) fail(409, 'conflict', 'email already taken', 'email');
    const u = { id: next(c, 'user'), email, name: b.name.trim(), password: b.password };
    c.s.users.push(u); return json({ id: u.id, email, name: u.name }, 201);
  }
  if (path === '/sessions' && m === 'POST') {
    const b = await body(req); const email = typeof b.email === 'string' ? b.email.toLowerCase() : '';
    const u = c.s.users.find(x => x.email === email && x.password === b.password);
    if (!u) return fail(401, 'unauthorized', 'invalid email or password');
    const token = `session-${next(c, 'token')}`;
    c.s.sessions.set(token, u.id); return json({ token }, 201);
  }
  if (path === '/sessions' && m === 'DELETE') {
    const u = user(c, req); void u;
    const token = req.headers.get('Authorization')!.slice(7); c.s.sessions.delete(token); return json(null, 204);
  }
  if (path === '/me' && m === 'GET') {
    const u = user(c, req); return json({ id: u.id, email: u.email, name: u.name });
  }
  return null;
}
