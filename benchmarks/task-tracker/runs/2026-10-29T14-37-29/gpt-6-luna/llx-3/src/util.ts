import type { State } from "./state";
export const json = (body: any, status = 200, headers: Record<string, string> = {}) =>
  Response.json(body, { status, headers });
export const err = (status: number, code: string, message: string, field?: string) =>
  json({ error: { code, message, ...(field ? { field } : {}) } }, status);
export const bad = (field: string) => err(400, "validation", `Invalid ${field}`, field);
export const obj = (v: any) => !!v && typeof v === "object" && !Array.isArray(v);
export async function body(req: Request): Promise<any> {
  try { const v = await req.json(); return obj(v) ? v : null; } catch { return null; }
}
export const iso = (s: State) => new Date(s.now()).toISOString();
export const userView = (u: any) => ({ id: u.id, email: u.email, name: u.name });
export function auth(req: Request, s: State) {
  const m = /^Bearer (.+)$/.exec(req.headers.get("Authorization") ?? "");
  return m ? s.users.find(u => u.id === s.sessions.get(m[1]!)) : undefined;
}
export const member = (p: any, uid: number) => p && (p.ownerId === uid ||
  p.members.some((m: any) => m.userId === uid));
export function taskByKey(s: State, key: string) {
  return s.tasks.find(t => t.key === key && !t.deleted);
}
export function taskReply(t: any, headers: Record<string, string> = {}) {
  return json(t, 200, { ETag: `"${t.version}"`, ...headers });
}
export function record(s: State, t: any, by: number, field: string, from: any, to: any) {
  s.history.push({ taskId: t.id, at: iso(s), by, field, from, to });
}
