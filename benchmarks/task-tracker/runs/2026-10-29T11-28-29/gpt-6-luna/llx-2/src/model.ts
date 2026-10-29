export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string; updatedAt: string;
  version: number;
};
export type State = {
  now: () => number; users: User[]; projects: Project[]; tasks: Task[];
  members: Map<number, Map<number, string>>; tokens: Map<string, number>;
  comments: Map<number, any[]>; history: Map<number, any[]>; deps: Map<number, Set<number>>;
  limits: Map<string, { start: number; count: number }>;
  idem: Map<string, { at: number; body: string; result: Task }>;
  taskNumbers: Map<number, number>;
  ids: { user: number; project: number; task: number; comment: number; token: number };
};
export const iso = (s: State) => new Date(s.now()).toISOString();
export function error(code: string, message: string, status: number, field?: string, headers?: Headers) {
  const detail: Record<string, string> = { code, message };
  if (field !== undefined) detail.field = field;
  return Response.json({ error: detail }, { status, headers });
}
export const bad = (field: string) => error("validation", `Invalid ${field}`, 400, field);
export const notFound = () => error("not_found", "Not found", 404);
export function bodyObject(value: unknown): Record<string, any> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, any> : null;
}
export const membership = (s: State, p: Project, u: number) =>
  s.members.get(p.id)?.get(u) ?? (p.ownerId === u ? "owner" : undefined);
export function visibleTask(s: State, key: string, user: number) {
  const t = s.tasks.find(x => x.key === key);
  const p = t && s.projects.find(x => x.id === t.projectId);
  return t && p && membership(s, p, user) ? t : undefined;
}
