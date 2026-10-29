import type { Ctx } from './model';
import { next } from './model';
import { body, fail, json, need, project, role, user } from './http';
export async function projectRoute(c: Ctx, req: Request, path: string) {
  const m = req.method;
  if (path === '/projects' && m === 'GET') {
    const u = user(c, req); return json(c.s.projects.filter(p => c.s.members.get(p.id)?.has(u.id)));
  }
  if (path === '/projects' && m === 'POST') {
    const u = user(c, req), b = await body(req);
    need(typeof b.name === 'string' && b.name.trim().length > 0 && b.name.trim().length <= 100, 'name');
    need(typeof b.key === 'string' && /^[A-Z]{2,6}$/.test(b.key), 'key');
    if (c.s.projects.some(p => p.key === b.key)) fail(409, 'conflict', 'key already taken', 'key');
    const p = { id: next(c, 'project'), name: b.name.trim(),
      key: b.key, ownerId: u.id, createdAt: new Date(c.now()).toISOString() };
    c.s.projects.push(p); c.s.members.set(p.id, new Map([[u.id, 'owner']])); return json(p, 201);
  }
  const member = path.match(/^\/projects\/(\d+)\/members(?:\/(\d+))?$/);
  if (member) {
    const u = user(c, req), p = project(c, member[1]!, u), members = c.s.members.get(p.id)!;
    if (member[2] && m === 'DELETE') {
      if (u.id !== p.ownerId) fail(403, 'forbidden', 'owner access required');
      const id = Number(member[2]);
      if (!members.has(id) || id === p.ownerId) fail(404, 'not_found', 'member not found');
      members.delete(id);
      for (const t of c.s.tasks.filter(x => x.projectId === p.id && x.assigneeId === id)) {
        t.assigneeId = null;
      }
      return json(null, 204);
    }
    if (!member[2] && m === 'GET') {
      role(c, p, u);
      return json([...members].sort((a, b) => a[0] - b[0])
        .map(([userId, r]) => ({ userId, role: r })));
    }
    if (!member[2] && m === 'POST') {
      if (u.id !== p.ownerId) fail(403, 'forbidden', 'owner access required');
      const b = await body(req); need(b.role === 'member' || b.role === 'viewer', 'role');
      need(Number.isInteger(b.userId) && c.s.users.some(x => x.id === b.userId), 'userId');
      if (members.has(b.userId)) fail(409, 'conflict', 'already a member');
      members.set(b.userId, b.role); return json({ userId: b.userId, role: b.role }, 201);
    }
  }
  const match = path.match(/^\/projects\/(\d+)$/);
  if (match && m === 'GET') { const u = user(c, req); return json(project(c, match[1]!, u)); }
  return null;
}
