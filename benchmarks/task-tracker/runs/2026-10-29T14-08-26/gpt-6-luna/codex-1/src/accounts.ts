import type { State } from './model';
import { stamp } from './model';
import { bad, conflict, empty, fail, forbidden, member, missing, ok } from './http';
export function accountRoute(s: State, method: string, path: string, userId: number | undefined,
  body: any, token: string | undefined): any | undefined {
  if (path === '/users') {
    if (method !== 'POST') return undefined;
    const email = typeof body?.email === 'string' ? body.email.trim().toLowerCase() : '';
    if (!/^\S+@\S+\.\S+$/.test(email)) return bad('email');
    if (typeof body.password !== 'string' || body.password.length < 8) return bad('password');
    const name = typeof body.name === 'string' ? body.name.trim() : '';
    if (!name || name.length > 100) return bad('name');
    if (s.users.some(u => u.email === email)) return { ...conflict(), body: { error: {
      code: 'conflict', message: 'Already exists', field: 'email' } } };
    const user = { id: ++s.ids.user, email, name, password: body.password };
    s.users.push(user);
    return ok({ id: user.id, email, name }, 201);
  }
  if (path === '/sessions') {
    if (method !== 'POST') return undefined;
    const email = typeof body?.email === 'string' ? body.email.toLowerCase() : '';
    const u = s.users.find(x => x.email === email && x.password === body?.password);
    if (!u) return fail(401, 'unauthorized', 'Invalid email or password');
    const value = `session-${u.id}-${s.now()}-${Math.random()}`;
    s.tokens.set(value, u.id);
    return ok({ token: value }, 201);
  }
  if (path === '/me') {
    if (method !== 'GET') return undefined;
    if (!userId) return fail(401, 'unauthorized', 'Sign in required');
    const u = s.users.find(x => x.id === userId)!;
    return ok({ id: u.id, email: u.email, name: u.name });
  }
  if (path === '/sessions' && method === 'DELETE') {
    if (!token) return fail(401, 'unauthorized', 'Sign in required');
    s.tokens.delete(token);
    return empty();
  }
  return undefined;
}
export function projectRoute(s: State, method: string, path: string, userId: number | undefined,
  body: any): any | undefined {
  if (path === '/projects') {
    if (!userId) return fail(401, 'unauthorized', 'Sign in required');
    if (method === 'GET') return ok(s.projects.filter(p => member(s, p.id, userId)));
    if (method !== 'POST') return undefined;
    const name = typeof body?.name === 'string' ? body.name.trim() : '';
    if (!name || name.length > 100) return bad('name');
    if (typeof body.key !== 'string' || !/^[A-Z]{2,6}$/.test(body.key)) return bad('key');
    if (s.projects.some(p => p.key === body.key)) return { status: 409, body: { error: {
      code: 'conflict', message: 'Already exists', field: 'key' } } };
    const p = { id: ++s.ids.project, name, key: body.key, ownerId: userId, createdAt: stamp(s) };
    s.projects.push(p);
    s.members.set(p.id, new Map([[userId, 'owner']]));
    return ok(p, 201);
  }
  const m = path.match(/^\/projects\/([^/]+)(?:\/(members)(?:\/(\d+))?)?$/);
  if (!m) return undefined;
  if (!userId) return fail(401, 'unauthorized', 'Sign in required');
  const p = s.projects.find(x => String(x.id) === m[1]);
  if (!p || !member(s, p.id, userId)) return missing();
  if (!m[2]) return method === 'GET' ? ok(p) : undefined;
  const members = s.members.get(p.id)!;
  if (m[3]) {
    if (method !== 'DELETE') return undefined;
    if (members.get(userId) !== 'owner') return forbidden();
    const target = Number(m[3]);
    if (!members.has(target) || target === p.ownerId) return missing();
    members.delete(target);
    for (const t of s.tasks.filter(x => x.projectId === p.id && x.assigneeId === target)) t.assigneeId = null;
    return empty();
  }
  if (method === 'GET') {
    const rows = [...members].map(([uid, role]) => ({ userId: uid, role }));
    rows.sort((a, b) => (a.role === 'owner' ? -1 : b.role === 'owner' ? 1 : a.userId - b.userId));
    return ok(rows);
  }
  if (method !== 'POST') return undefined;
  if (members.get(userId) !== 'owner') return forbidden();
  if (!s.users.some(u => u.id === body?.userId)) return bad('userId');
  if (body.role !== 'member' && body.role !== 'viewer') return bad('role');
  if (members.has(body.userId)) {
    return { status: 409, body: { error: { code: 'conflict', message: 'Already a member' } } };
  }
  members.set(body.userId, body.role);
  return ok({ userId: body.userId, role: body.role }, 201);
}
