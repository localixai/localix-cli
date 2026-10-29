import type { State, Task } from './model';
export type Result = { status: number; body?: unknown; headers?: Record<string, string> };
export const ok = (body: unknown, status = 200, headers: Record<string, string> = {}): Result =>
  ({ status, body, headers });
export const empty = (status = 204): Result => ({ status });
export const fail = (status: number, code: string, message: string, field?: string): Result =>
  ({ status, body: { error: { code, message, ...(field ? { field } : {}) } } });
export const bad = (field: string, message = `Invalid ${field}`) => fail(400, 'validation', message, field);
export const missing = () => fail(404, 'not_found', 'Not found');
export const forbidden = () => fail(403, 'forbidden', 'Forbidden');
export const conflict = (code = 'conflict') => fail(409, code, 'Conflict');
export function member(s: State, projectId: number, userId: number) {
  return s.members.get(projectId)?.get(userId);
}
export function taskFor(s: State, key: string): Task | undefined {
  return s.tasks.find(t => t.key === key && !t.deleted);
}
export function projectForTask(s: State, task: Task) {
  return s.projects.find(p => p.id === task.projectId);
}
export function allowed(s: State, task: Task, user: number) {
  return Boolean(member(s, task.projectId, user));
}
export function roleCanWrite(s: State, projectId: number, user: number) {
  const role = member(s, projectId, user);
  return role === 'owner' || role === 'member';
}
export function etag(task: Task) { return { ETag: `"${task.version}"` }; }
