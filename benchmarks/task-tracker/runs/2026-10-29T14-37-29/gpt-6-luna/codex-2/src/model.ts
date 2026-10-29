export type User = {
  id: number;
  email: string;
  password: string;
  name: string;
};
export type Project = {
  id: number;
  name: string;
  key: string;
  ownerId: number;
  createdAt: string;
};
export type Task = {
  id: number;
  key: string;
  number: number;
  projectId: number;
  title: string;
  description: string;
  status: string;
  priority: string;
  labels: string[];
  assigneeId: number | null;
  createdBy: number;
  createdAt: string;
  updatedAt: string;
  version: number;
};
export type State = {
  users: User[];
  projects: Project[];
  tasks: Task[];
  sessions: Map<string, number>;
  members: Map<number, Map<number, string>>;
  histories: Map<number, any[]>;
  comments: Map<number, any[]>;
  deps: Map<number, Set<number>>;
  counters: Record<string, number>;
  idempotency: Map<string, { at: number; body: string; value: any }>;
  limits: Map<string, { minute: number; count: number }>;
};
export type Context = {
  state: State;
  now: () => number;
  request: Request;
  userId?: number;
};
export type Result = {
  status?: number;
  body?: any;
  headers?: Record<string, string>;
};
export const json = (
  body: any,
  status = 200,
  headers: Record<string, string> = {},
): Result => ({ status, body, headers });
export const fail = (
  code: string,
  message: string,
  status: number,
  field?: string,
): Result =>
  json({ error: { code, message, ...(field ? { field } : {}) } }, status);
export const nextId = (s: State, name: string) =>
  (s.counters[name] = (s.counters[name] ?? 0) + 1);
export const iso = (c: Context) => new Date(c.now()).toISOString();
export const memberRole = (s: State, projectId: number, userId: number) =>
  s.members.get(projectId)?.get(userId);
export function visibleProject(c: Context, id: number) {
  return c.state.projects.find(
    (p) => p.id === id && memberRole(c.state, id, c.userId!) !== undefined,
  );
}
export function taskByKey(s: State, key: string) {
  return s.tasks.find((t) => t.key === key);
}
export function errorNotFound() {
  return fail("not_found", "not found", 404);
}
export function validateBody(body: any): boolean {
  return !!body && typeof body === "object" && !Array.isArray(body);
}
