import { error, type State } from "./model";

export type Ctx = { state: State; user: number | null; body: any; url: URL; req: Request };
export type Result = { status: number; body?: any; headers?: Record<string, string> };
export const ok = (body: any, status = 200, headers: Record<string, string> = {}): Result =>
  ({ status, body, headers });
export const fail = (status: number, code: string, message = code, field?: string): Result =>
  ok(error(code, message, field), status);
export async function readBody(req: Request): Promise<any> {
  if (!["POST", "PATCH", "PUT"].includes(req.method)) return {};
  try {
    const v = await req.json();
    return v && typeof v === "object" && !Array.isArray(v) ? v : null;
  } catch { return null; }
}
export function projectAccess(c: Ctx, id: number) {
  const p = c.state.projects.find(x => x.id === id);
  if (!p || !p.members.has(c.user!)) return null;
  return p;
}
export function taskAccess(c: Ctx, key: string) {
  const t = c.state.tasks.find(x => x.key === key && !x.deleted);
  if (!t || !projectAccess(c, t.projectId)) return null;
  return t;
}
export function memberRole(c: Ctx, p: { members: Map<number, string> }) {
  return p.members.get(c.user!) ?? "";
}
export function labels(v: any): string[] | null {
  if (!Array.isArray(v) || v.some(x => typeof x !== "string" ||
    !x.trim() || x.trim().length > 30 || /\s/.test(x.trim()))) return null;
  return [...new Set(v.map(x => x.trim().toLowerCase()))].sort();
}
export const string = (v: any) => typeof v === "string";
