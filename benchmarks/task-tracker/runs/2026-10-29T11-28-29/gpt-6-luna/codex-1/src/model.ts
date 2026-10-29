export type User = {
  id: number;
  email: string;
  name: string;
  password: string;
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
  deleted?: boolean;
};
export type State = {
  users: User[];
  projects: Project[];
  tasks: Task[];
  sessions: Map<string, number>;
  members: Map<number, Map<number, string>>;
  comments: Map<number, any[]>;
  history: Map<number, any[]>;
  deps: Map<number, Set<number>>;
  idempotency: Map<string, any>;
  limits: Map<string, { start: number; count: number }>;
  ids: { user: number; project: number; task: number; comment: number };
  now: () => number;
};
export const stamp = (s: State) => new Date(s.now()).toISOString();
export const json = (
  body: unknown,
  status = 200,
  headers?: Record<string, string>,
) =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      "Content-Type": "application/json",
      ...Object.fromEntries(new Headers(headers)),
    },
  });
export const fail = (
  status: number,
  code: string,
  message = code,
  field?: string,
) => json({ error: { code, message, ...(field ? { field } : {}) } }, status);
export const err = (field: string, message = `Invalid ${field}`) =>
  fail(400, "validation", message, field);
export const safe = (u: User) => ({ id: u.id, email: u.email, name: u.name });
export const taskView = (t: Task) => {
  const { deleted, ...view } = t;
  return view;
};
export function role(s: State, p: Project, uid: number) {
  return p.ownerId === uid ? "owner" : s.members.get(p.id)?.get(uid);
}
export function taskByKey(s: State, key: string) {
  return s.tasks.find((t) => t.key === key && !t.deleted);
}
export function projectAccess(s: State, uid: number, id: number) {
  const p = s.projects.find((x) => x.id === id);
  return p && role(s, p, uid) ? p : undefined;
}
export const canWrite = (r?: string) => r === "owner" || r === "member";
export function labels(value: unknown): string[] | null {
  if (
    !Array.isArray(value) ||
    value.some(
      (x) =>
        typeof x !== "string" || x.trim().length > 30 || /\s/.test(x.trim()),
    )
  )
    return null;
  return [...new Set(value.map((x) => x.trim().toLowerCase()))].sort();
}
