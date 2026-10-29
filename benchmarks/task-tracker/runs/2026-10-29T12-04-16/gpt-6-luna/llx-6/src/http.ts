import type { State, Task } from "./model";
export type Ctx = { s: State; request: Request; url: URL; user: number | null };
export class Fault extends Error {
  constructor(public status: number, public code: string, message: string, public field?: string) {
    super(message);
  }
}
export const fail = (status: number, code: string, message: string, field?: string): never => {
  throw new Fault(status, code, message, field);
};
export const ok = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  Response.json(body, { status, headers });
export const noContent = () => new Response(null, { status: 204 });
export async function body(c: Ctx): Promise<Record<string, any>> {
  let value: any;
  try { value = await c.request.json(); } catch { return fail(400, "validation", "Invalid JSON"); }
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return fail(400, "validation", "Body must be an object");
  }
  return value;
}
export const valid = (condition: unknown, field: string, message = `Invalid ${field}`): void => {
  if (!condition) fail(400, "validation", message, field);
};
export function auth(c: Ctx, role?: "member" | "owner", projectId?: number): number {
  if (!c.user) return fail(401, "unauthorized", "Authentication required");
  if (projectId !== undefined) {
    const members = c.s.members.get(projectId);
    if (!members?.has(c.user)) return fail(404, "not_found", "Project not found");
    const r = members.get(c.user);
    if (role === "owner" && r !== "owner") return fail(403, "forbidden", "Owner required");
    if (role === "member" && r === "viewer") return fail(403, "forbidden", "Member required");
  }
  return c.user;
}
export function taskByKey(c: Ctx, key: string): Task {
  const t = c.s.tasks.find((x) => x.key === key);
  if (!t) return fail(404, "not_found", "Task not found");
  auth(c, undefined, t.projectId);
  return t;
}
export const etag = (t: Task) => ({ ETag: `"${t.version}"` });
export function match(c: Ctx, t: Task) {
  const v = c.request.headers.get("If-Match");
  if (v && v !== `"${t.version}"`) fail(412, "precondition_failed", "Task has changed");
}
export function changed(c: Ctx, t: Task, user: number, field: string, from: any, to: any) {
  (c.s.history.get(t.key) ?? c.s.history.set(t.key, []).get(t.key)!).push({
    at: new Date(c.s.now()).toISOString(), by: user, field, from, to,
  });
}
