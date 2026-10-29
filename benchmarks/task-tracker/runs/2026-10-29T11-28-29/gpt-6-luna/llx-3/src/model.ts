export type User = { id: number; email: string; password: string; name: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string; description: string;
  status: string; priority: string; labels: string[]; assigneeId: number | null; createdBy: number;
  createdAt: string; updatedAt: string; version: number;
};
export type State = {
  now: () => number; users: User[]; projects: Project[]; tasks: Task[];
  members: Map<number, Map<number, string>>; tokens: Map<string, number>;
  comments: Map<number, any[]>; history: Map<number, any[]>; deps: Map<number, Set<number>>;
  limits: Map<string, { start: number; count: number }>;
  idem: Map<string, { at: number; body: string; result: any }>;
  taskCounters: Map<number, number>;
  uid: number; pid: number; tid: number; tokenId: number;
};
export const iso = (s: State) => new Date(s.now()).toISOString();
export function error(code: string, message: string, status: number, field?: string) {
  const detail: any = { code, message };
  if (field !== undefined) detail.field = field;
  return Response.json({ error: detail }, { status });
}
export const bad = (field: string) => error("validation", `Invalid ${field}`, 400, field);
export const conflict = (field?: string) => error("conflict", "Already exists", 409, field);
export const json = (body: unknown, status = 200, headers?: Record<string, string>) =>
  Response.json(body, { status, headers });
export function memberRole(s: State, p: Project, uid: number) {
  return s.members.get(p.id)?.get(uid);
}
export function byTaskKey(s: State, key: string) { return s.tasks.find((t) => t.key === key); }
export function projectForTask(s: State, t: Task) { return s.projects.find((p) => p.id === t.projectId)!; }
