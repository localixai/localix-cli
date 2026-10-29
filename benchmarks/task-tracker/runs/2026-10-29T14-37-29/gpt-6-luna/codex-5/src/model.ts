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
  next: number;
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
  depends: number[];
  comments: any[];
  history: any[];
};
export type DB = {
  users: User[];
  projects: Project[];
  tasks: Task[];
  sessions: Map<string, number>;
  members: Map<number, Map<number, string>>;
  idem: Map<string, any>;
  limits: Map<string, { minute: number; count: number }>;
};
export type Ctx = {
  db: DB;
  now: () => number;
  user: User | null;
  req: Request;
  url: URL;
};
export const json = (
  body: unknown,
  status = 200,
  headers: Record<string, string> = {},
) => Response.json(body, { status, headers });
export function err(
  code: string,
  status: number,
  field?: string,
  headers: Record<string, string> = {},
): Response {
  const error: Record<string, string> = {
    code,
    message: code.replaceAll("_", " "),
  };
  if (field) error.field = field;
  return json({ error }, status, headers);
}
export const validation = (field?: string) => err("validation", 400, field);
export const auth = (c: Ctx) => {
  const token = c.req.headers.get("Authorization")?.match(/^Bearer (.+)$/)?.[1];
  const id = token ? c.db.sessions.get(token) : undefined;
  return id ? (c.db.users.find((u) => u.id === id) ?? null) : null;
};
export function projectFor(c: Ctx, id: number) {
  const p = c.db.projects.find((x) => x.id === id);
  if (!p || !c.user || !c.db.members.get(id)?.has(c.user.id)) return null;
  return p;
}
export function taskFor(c: Ctx, key: string) {
  const t = c.db.tasks.find((x) => x.key === key && !x.deleted);
  return t && projectFor(c, t.projectId) ? t : null;
}
export const role = (c: Ctx, p: Project) =>
  c.db.members.get(p.id)?.get(c.user!.id);
export const iso = (c: Ctx) => new Date(c.now()).toISOString();
export async function body(c: Ctx): Promise<any | null> {
  try {
    const x = await c.req.json();
    return x && typeof x === "object" && !Array.isArray(x) ? x : null;
  } catch {
    return null;
  }
}
export function labels(value: unknown): string[] | null {
  if (
    !Array.isArray(value) ||
    value.some(
      (x) =>
        typeof x !== "string" ||
        !x.trim() ||
        x.trim().length > 30 ||
        /\s/.test(x.trim()),
    )
  )
    return null;
  return [...new Set(value.map((x) => x.trim().toLowerCase()))].sort();
}
