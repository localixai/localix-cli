import type { Store, User, Project, Task } from "./model";

export const json = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  Response.json(body, { status, headers });
export const error = (code: string, message = code, status = 400, field?: string) =>
  json({ error: { code, message, ...(field ? { field } : {}) } }, status);
export const bad = (field: string) => error("validation", `invalid ${field}`, 400, field);
export async function body(req: Request): Promise<Record<string, any> | null> {
  try {
    const value = await req.json();
    return value && typeof value === "object" && !Array.isArray(value) ? value : null;
  } catch { return null; }
}
export function auth(req: Request, s: Store): User | null {
  const match = /^Bearer (.+)$/.exec(req.headers.get("Authorization") || "");
  const id = match && s.tokens.get(match[1]!);
  return id ? s.users.find(u => u.id === id) || null : null;
}
export const member = (p: Project, u: User) => p.ownerId === u.id || p.members.has(u.id);
export function project(s: Store, id: string): Project | undefined {
  return /^\d+$/.test(id) ? s.projects.find(p => p.id === Number(id)) : undefined;
}
export function task(s: Store, key: string): Task | undefined {
  return s.tasks.find(t => t.key === key && !t.deleted);
}
export function precondition(req: Request, t: Task) {
  const match = req.headers.get("If-Match");
  return match && match !== `"${t.version}"` ? error("precondition_failed", "task has changed", 412) : null;
}
export function role(p: Project, u: User): string | undefined {
  return p.ownerId === u.id ? "owner" : p.members.get(u.id);
}
