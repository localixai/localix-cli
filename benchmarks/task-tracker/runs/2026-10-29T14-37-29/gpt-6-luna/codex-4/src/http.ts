import type { Ctx, User, Project, Task } from './model';
export class Fault extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string) { super(message); }
}
export const fail = (status: number, code: string, message: string, field?: string): never => {
  throw new Fault(status, code, message, field);
};
export const need = (ok: unknown, field: string, message = 'is invalid'): void => {
  if (!ok) fail(400, 'validation', `${field} ${message}`, field);
};
export function user(c: Ctx, req: Request): User {
  const token = req.headers.get('Authorization')?.match(/^Bearer (.+)$/)?.[1];
  const id = token ? c.s.sessions.get(token) : undefined;
  const found = c.s.users.find(x => x.id === id);
  if (!found) return fail(401, 'unauthorized', 'authentication required');
  return found;
}
export function project(c: Ctx, id: string, u: Pick<User, 'id'>): Project {
  const p = c.s.projects.find(x => x.id === Number(id));
  if (!p || !c.s.members.get(p.id)?.has(u.id)) return fail(404, 'not_found', 'project not found');
  return p;
}
export function task(c: Ctx, key: string, u: Pick<User, 'id'>): Task {
  const t = c.s.tasks.find(x => `${c.s.projects.find(p => p.id === x.projectId)?.key}-${x.number}` === key);
  if (!t || !c.s.members.get(t.projectId)?.has(u.id)) return fail(404, 'not_found', 'task not found');
  return t;
}
export function role(c: Ctx, p: Project, u: Pick<User, 'id'>, write = false) {
  const r = c.s.members.get(p.id)?.get(u.id);
  if (!r) fail(404, 'not_found', 'project not found');
  if (write && r === 'viewer') fail(403, 'forbidden', 'write access required');
  return r;
}
export async function body(req: Request): Promise<Record<string, any>> {
  let v: unknown;
  try { v = await req.json(); } catch { fail(400, 'validation', 'body must be JSON'); }
  if (!v || typeof v !== 'object' || Array.isArray(v)) fail(400, 'validation', 'body must be an object');
  return v as Record<string, any>;
}
export const taskView = (c: Ctx, t: Task) => ({
  ...t, key: `${c.s.projects.find(p => p.id === t.projectId)!.key}-${t.number}`,
});
export function json(data: unknown, status = 200, headers: Record<string, string> = {}) {
  return new Response(data === null ? null : JSON.stringify(data), { status, headers: {
    ...(data === null ? {} : { 'Content-Type': 'application/json; charset=utf-8' }), ...headers,
  }});
}
export function errorResponse(e: unknown) {
  const f = e instanceof Fault ? e : new Fault(500, 'internal', 'internal server error');
  const error: Record<string, string> = { code: f.code, message: f.message };
  if (f.field) error.field = f.field;
  return json({ error }, f.status);
}
export function etag(t: Task) { return { ETag: `"${t.version}"` }; }
export function checkVersion(req: Request, t: Task) {
  const v = req.headers.get('If-Match');
  if (v && v !== `"${t.version}"`) fail(412, 'precondition_failed', 'task version changed');
}
