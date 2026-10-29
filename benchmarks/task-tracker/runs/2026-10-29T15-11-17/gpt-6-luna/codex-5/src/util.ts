import type { State, Task } from "./model";
export type Ctx = { state: State; now: () => number; request: Request };
export class ApiError extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string) {
    super(message);
  }
}
export const fail = (status: number, code: string, message: string, field?: string): never => {
  throw new ApiError(status, code, message, field);
};
export function json(body: unknown, status = 200, headers: Record<string, string> = {}) {
  return Response.json(body, { status, headers });
}
export async function body(req: Request): Promise<Record<string, any>> {
  try {
    const val = await req.json();
    if (val && typeof val === "object" && !Array.isArray(val)) return val;
  } catch {}
  fail(400, "validation", "Body must be a JSON object");
  return {};
}
export function auth(ctx: Ctx, required = true) {
  const token = /^Bearer (.+)$/.exec(ctx.request.headers.get("Authorization") || "")?.[1];
  const id = token ? ctx.state.tokens.get(token) : undefined;
  if (!id && required) fail(401, "unauthorized", "Authentication required");
  return { token, user: ctx.state.users.find((u) => u.id === id) };
}
export function project(ctx: Ctx, id: number) {
  const p = ctx.state.projects.find((x) => x.id === id);
  const user = auth(ctx).user!;
  if (!p || !ctx.state.members.get(id)?.has(user.id)) fail(404, "not_found", "Project not found");
  return p!;
}
export function role(ctx: Ctx, id: number) {
  const u = auth(ctx).user!;
  return ctx.state.members.get(id)?.get(u.id);
}
export function task(ctx: Ctx, key: string): Task {
  const t = ctx.state.tasks.find((x) => {
    const p = ctx.state.projects.find((y) => y.id === x.projectId);
    return `${p?.key}-${x.number}` === key;
  });
  if (!t || !ctx.state.members.get(t.projectId)?.has(auth(ctx).user!.id)) {
    fail(404, "not_found", "Task not found");
  }
  return t!;
}
export function validate(condition: unknown, field: string, message = "Invalid value") {
  if (!condition) fail(400, "validation", message, field);
}
export function date(ctx: Ctx) { return new Date(ctx.now()).toISOString(); }
export function etag(t: Task) { return { ETag: `"${t.version}"` }; }
export function writeTask(ctx: Ctx, t: Task, fields: Record<string, any>, by: number) {
  const at = date(ctx); const history = ctx.state.histories.get(taskId(t)) || [];
  let changed = false;
  for (const field of Object.keys(fields).sort()) {
    const old = (t as any)[field], value = fields[field];
    if (JSON.stringify(old) === JSON.stringify(value)) continue;
    history.push({ at, by, field, from: old, to: value });
    (t as any)[field] = value; changed = true;
  }
  if (changed) { t.version++; t.updatedAt = at; }
  ctx.state.histories.set(taskId(t), history);
  return changed;
}
export function taskId(t: Task) { return `${t.projectId}:${t.number}`; }
