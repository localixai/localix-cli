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
  seq: number;
  members: Map<number, string>;
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
  deps: Set<string>;
  history: any[];
  comments: any[];
};
export type State = {
  users: User[];
  projects: Project[];
  tasks: Task[];
  sessions: Map<string, number>;
  idem: Map<string, any>;
  hits: Map<string, number[]>;
  counters: { user: number; project: number; task: number; comment: number };
  now: () => number;
};
export class Fault extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
    public field?: string,
  ) {
    super(message);
  }
}
export function fail(
  status: number,
  code: string,
  message = code,
  field?: string,
): never {
  throw new Fault(status, code, message, field);
}
export const iso = (s: State) => new Date(s.now()).toISOString();
export const person = (u: User) => ({ id: u.id, email: u.email, name: u.name });
export function json(
  body: any,
  status = 200,
  headers: Record<string, string> = {},
) {
  return Response.json(body, { status, headers });
}
export const err = (e: Fault, extra: Record<string, string> = {}) =>
  json(
    {
      error: {
        code: e.code,
        message: e.message,
        ...(e.field ? { field: e.field } : {}),
      },
    },
    e.status,
    extra,
  );
export function parseKey(key: string, s: State): Task | undefined {
  return s.tasks.find((t) => t.key === key);
}
export function projectFor(s: State, id: number, user: number) {
  const p = s.projects.find((x) => x.id === id);
  if (!p || !p.members.has(user)) fail(404, "not_found");
  return p;
}
export function memberRole(p: Project, u: number) {
  return p.members.get(u);
}
export function taskAccess(s: State, key: string, user: number) {
  const t = parseKey(key, s);
  if (!t) fail(404, "not_found");
  const p = projectFor(s, t.projectId, user);
  return { t, p };
}
export function requireRole(p: Project, u: number) {
  if (p.members.get(u) === "viewer") fail(403, "forbidden");
}
export async function body(req: Request) {
  try {
    const b = await req.json();
    if (!b || typeof b !== "object" || Array.isArray(b))
      fail(400, "validation", "body must be an object");
    return b as Record<string, any>;
  } catch (e) {
    if (e instanceof Fault) throw e;
    fail(400, "validation", "body must be valid JSON");
  }
}
export function validate(ok: boolean, field: string): asserts ok {
  if (!ok) fail(400, "validation", `invalid ${field}`, field);
}
