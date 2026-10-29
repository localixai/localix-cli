import type { Ctx, Project, Task } from "./types";

export const json = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  Response.json(body, { status, headers });
export const fail = (code: string, message: string, status: number, field?: string) =>
  json({ error: { code, message, ...(field ? { field } : {}) } }, status);
export const validation = (field: string) => fail("validation", `Invalid ${field}`, 400, field);
export const malformed = () => fail("validation", "Invalid request body", 400);
export const denied = () => fail("forbidden", "Forbidden", 403);
export const missing = () => fail("not_found", "Not found", 404);
export const conflict = (field?: string) => fail("conflict", "Conflict", 409, field);
export const stamp = (ctx: Ctx) => new Date(ctx.store.now()).toISOString();
export async function body(req: Request): Promise<Record<string, any> | null> {
  try {
    const value = await req.json();
    return value && typeof value === "object" && !Array.isArray(value) ? value : null;
  } catch { return null; }
}
export const member = (p: Project, id: number) => p.members.get(id);
export const visibleProject = (ctx: Ctx, id: number) =>
  ctx.store.projects.find(p => p.id === id && ctx.user && p.members.has(ctx.user.id));
export const taskByKey = (ctx: Ctx, key: string) =>
  ctx.store.tasks.find(t => t.key === key && !t.deleted);
export const visibleTask = (ctx: Ctx, key: string) => {
  const task = taskByKey(ctx, key);
  return task && visibleProject(ctx, task.projectId) ? task : undefined;
};
export function canWrite(ctx: Ctx, task: Task) {
  const p = ctx.store.projects.find(x => x.id === task.projectId)!;
  return ctx.user && ["owner", "member"].includes(p.members.get(ctx.user.id)?.role ?? "");
}
export function etag(task: Task) { return `"${task.version}"`; }
export function taskReply(task: Task, status = 200) {
  return json(Object.fromEntries(Object.entries(task).filter(([k]) =>
    !["deleted", "dependencies"].includes(k))), status, { ETag: etag(task) });
}
export function normalizeLabels(value: unknown): string[] | null {
  if (!Array.isArray(value) || value.some(x => typeof x !== "string")) return null;
  const labels = value.map(x => x.trim().toLowerCase());
  if (labels.some(x => !/^[a-z0-9-]{1,30}$/.test(x))) return null;
  return [...new Set(labels)].sort();
}
