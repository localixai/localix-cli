export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string; description: string;
  status: string; priority: string; labels: string[]; assigneeId: number | null; createdBy: number;
  createdAt: string; updatedAt: string; version: number;
};
export type State = {
  users: User[]; projects: Project[]; memberships: Map<number, Map<number, string>>;
  tasks: Task[]; comments: Map<number, any[]>; history: Map<number, any[]>;
  tokens: Map<string, number>; limits: Map<string, { minute: number; count: number }>;
  now: () => number; ids: Record<string, number>;
  deps: Map<number, Set<number>>; idem: Map<string, { body: string; task: Task; at: number }>;
};
export type Ctx = { s: State; req: Request; url: URL; user?: User; body: any };
export const iso = (s: State) => new Date(s.now()).toISOString();
export const member = (s: State, p: Project, u: User) =>
  p.ownerId === u.id ? "owner" : s.memberships.get(p.id)?.get(u.id);
export const taskByKey = (s: State, key: string) => s.tasks.find(t => t.key === key);
export const err = (status: number, code: string, message = code, field?: string) => ({
  status, body: { error: { code, message, ...(field ? { field } : {}) } },
});
export function validate(ok: unknown, field: string, message = `Invalid ${field}`) {
  if (!ok) throw Object.assign(new Error(message), { field });
}
