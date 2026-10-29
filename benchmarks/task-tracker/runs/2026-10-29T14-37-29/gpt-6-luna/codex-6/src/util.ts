import type { Ctx, Result, Task } from "./types";
export const ok = (
  body?: any,
  status = 200,
  headers: Record<string, string> = {},
): Result => ({ status, body, headers });
export const fail = (
  status: number,
  code: string,
  message = code,
  field?: string,
): Result => ({
  status,
  body: { error: { code, message, ...(field ? { field } : {}) } },
});
export const iso = (c: Ctx) => new Date(c.state.now()).toISOString();
export function next(c: Ctx, name: string) {
  c.state.ids[name] = (c.state.ids[name] || 0) + 1;
  return c.state.ids[name];
}
export function member(c: Ctx, p: number) {
  return c.state.members.get(p)?.get(c.user!.id);
}
export function projectAccess(c: Ctx, p: number) {
  return c.state.projects.find((x) => x.id === p) && member(c, p);
}
export function findTask(c: Ctx, key: string): Task | undefined {
  return c.state.tasks.find((t) => t.key === key);
}
export function taskAccess(c: Ctx, key: string): Task | undefined {
  const t = findTask(c, key);
  return t && projectAccess(c, t.projectId) ? t : undefined;
}
export function jsonBody(c: Ctx): Result | null {
  try {
    const v = c.body;
    if (!v || typeof v !== "object" || Array.isArray(v))
      return fail(400, "validation", "body must be an object", "body");
    return null;
  } catch {
    return fail(400, "validation", "invalid JSON", "body");
  }
}
export function cleanLabels(v: any): string[] | null {
  if (!Array.isArray(v)) return null;
  const a: string[] = [];
  for (const x of v) {
    if (typeof x !== "string") return null;
    const s = x.trim().toLowerCase();
    if (!s || s.length > 30 || /\s/.test(s)) return null;
    a.push(s);
  }
  return [...new Set(a)].sort();
}
export const etag = (t: Task) => ({ ETag: `"${t.version}"` });
