export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; key: string; name: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string; description: string;
  status: string; priority: string; labels: string[]; assigneeId: number | null;
  createdBy: number; createdAt: string; updatedAt: string; version: number;
};
export type Context = {
  now: () => number; users: User[]; projects: Project[]; tasks: Task[];
  sessions: Map<string, number>; members: Map<number, Map<number, string>>;
  comments: any[]; dependencies: Map<number, Set<number>>; history: Map<number, any[]>;
  idem: Map<string, { body: string; time: number; result: Task }>;
  rates: Map<string, { minute: number; count: number }>;
  ids: { user: number; project: number; task: number; comment: number; number: Map<number, number> };
};
export const iso = (c: Context) => new Date(c.now()).toISOString();
export const json = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  Response.json(body, { status, headers });
export function error(status: number, code: string, message = code, field?: string) {
  const detail: Record<string, string> = { code, message };
  if (field) detail.field = field;
  return json({ error: detail }, status);
}
export const bad = (field: string) => error(400, "validation", `Invalid ${field}`, field);
export const conflict = (field: string) => error(409, "conflict", `${field} already exists`, field);
export function member(c: Context, p: Project, uid: number) {
  if (p.ownerId === uid) return "owner";
  return c.members.get(p.id)?.get(uid);
}
export function taskByKey(c: Context, key: string) { return c.tasks.find(t => t.key === key); }
export function cleanLabels(v: any) {
  return [...new Set((v as string[]).map(x => x.trim().toLowerCase()))].sort();
}
export function record(c: Context, t: Task, uid: number, field: string, from: any, to: any) {
  (c.history.get(t.id) ?? (c.history.set(t.id, []), c.history.get(t.id)!))
    .push({ at: iso(c), by: uid, field, from, to });
}
export function etag(body: unknown, status: number, version?: number) {
  return json(body, status, version ? { ETag: `"${version}"` } : {});
}
