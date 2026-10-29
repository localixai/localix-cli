import type { State } from "./model";

export const json = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  Response.json(body, { status, headers });
export function fail(code: string, message: string, status: number, field?: string) {
  const error: Record<string, string> = { code, message };
  if (field) error.field = field;
  return json({ error }, status);
}
export const bad = (field: string, message = `Invalid ${field}`) =>
  fail("validation", message, 400, field);
export const gone = () => fail("not_found", "Not found", 404);
export const forbidden = () => fail("forbidden", "Forbidden", 403);
export function member(s: State, projectId: number, userId: number) {
  return s.members.get(projectId)?.get(userId);
}
export function taskByKey(s: State, key: string) {
  return s.tasks.find(t => t.key === key);
}
export function changed(task: any, userId: number, now: string, field: string, from: any, to: any, s: State) {
  if (JSON.stringify(from) === JSON.stringify(to)) return;
  s.history.get(task.id)?.push({ at: now, by: userId, field, from, to });
}
export function normalizeLabels(value: unknown) {
  if (!Array.isArray(value) || value.some(x => typeof x !== "string" ||
    !x.trim() || x.trim().length > 30 || /\s/.test(x.trim()))) return null;
  return [...new Set(value.map(x => x.trim().toLowerCase()))].sort();
}
