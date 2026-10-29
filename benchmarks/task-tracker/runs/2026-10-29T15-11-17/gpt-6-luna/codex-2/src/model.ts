export type User = { id: number; email: string; name: string; password: string };
export type Project = {
  id: number;
  name: string;
  key: string;
  ownerId: number;
  createdAt: string;
  next: number;
};
export type Task = {
  id: number;
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
  deleted: boolean;
  deps: number[];
  history: any[];
};
export type State = {
  users: User[];
  projects: Project[];
  tasks: Task[];
  sessions: Map<string, number>;
  memberships: Map<string, string>;
  comments: Map<number, any[]>;
  idem: Map<string, any>;
  rates: Map<string, { start: number; count: number }>;
  now: () => number;
};
export const key = (p: number, u: number) => `${p}:${u}`;
export const iso = (s: State) => new Date(s.now()).toISOString();
export function err(
  code: string,
  message = code.replaceAll("_", " "),
  status = 400,
  field?: string,
): Response {
  const error: any = { code, message };
  if (field) error.field = field;
  return Response.json({ error }, { status });
}
export const ok = (body: any, status = 200, headers: Record<string, string> = {}) =>
  Response.json(body, { status, headers });
export const userView = (u: User) => ({ id: u.id, email: u.email, name: u.name });
export function publicTask(t: Task, p: Project) {
  const { deleted, deps, history, ...rest } = t;
  return { ...rest, key: `${p.key}-${t.number}` };
}
