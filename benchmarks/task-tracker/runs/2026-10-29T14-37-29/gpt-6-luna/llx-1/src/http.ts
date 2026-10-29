import type { State } from "./model";
export type Ctx = { s: State; req: Request; url: URL; user: number | null; now: string };
export const json = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  Response.json(body, { status, headers });
export const err = (code: string, message = code, field?: string, status = 400) =>
  json({ error: { code, message, ...(field ? { field } : {}) } }, status);
export async function body(c: Ctx): Promise<any> {
  try {
    const value = await c.req.json();
    return value && typeof value === "object" && !Array.isArray(value) ? value : null;
  } catch { return null; }
}
export function projectFor(c: Ctx, id: number) {
  const p = c.s.projects.find(x => x.id === id);
  return p && c.s.members.has(`${id}:${c.user}`) ? p : null;
}
export function role(c: Ctx, id: number) { return c.s.members.get(`${id}:${c.user}`); }
export function taskResponse(t: any, status = 200) {
  return json(t, status, { ETag: `"${t.version}"` });
}
export function member(c: Ctx, id: number, write = false) {
  const r = role(c, id);
  return r && (!write || r !== "viewer");
}
export function taskByKey(c: Ctx) {
  const key = c.url.pathname.split("/")[2];
  const t = c.s.tasks.find(x => x.key === key && !x.deleted);
  return t && projectFor(c, t.projectId) ? t : null;
}
export function match(c: Ctx, version: number) {
  const h = c.req.headers.get("If-Match");
  return !h || h === `"${version}"`;
}
