export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = { id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[]; assigneeId: number | null;
  createdBy: number; createdAt: string; updatedAt: string; version: number; deleted?: boolean };
export type State = {
  users: User[]; projects: Project[]; members: Map<number, Map<number, string>>;
  tasks: Task[]; histories: Map<number, any[]>; comments: Map<number, any[]>;
  deps: Map<number, Set<number>>; tokens: Map<string, number>; idempotency: Map<string, any>;
  limits: Map<string, { minute: number; count: number }>;
  ids: { user: number; project: number; task: number; comment: number };
  now: () => number;
};
export const stamp = (s: State) => new Date(s.now()).toISOString();
export const visible = (t: Task) => { const { deleted, ...out } = t; void deleted; return out; };
