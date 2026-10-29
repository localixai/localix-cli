export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string;
  updatedAt: string; version: number;
};
export type State = {
  users: User[]; projects: Project[]; tasks: Task[]; tokens: Map<string, number>;
  members: Map<string, string>; comments: Map<string, any[]>; history: Map<string, any[]>;
  deps: Map<number, Set<number>>; idem: Map<string, any>;
  hits: Map<string, { start: number; count: number }>;
  ids: { user: number; project: number; task: number; comment: number };
  sequences: Map<number, number>;
};
export type Ctx = { state: State; now: () => number };
export const pair = (p: number, u: number) => `${p}:${u}`;
export const iso = (ctx: Ctx) => new Date(ctx.now()).toISOString();
export const memberRole = (s: State, p: number, u: number): string | undefined =>
  s.projects.find(x => x.id === p)?.ownerId === u ? "owner" : s.members.get(pair(p, u));
export const taskByKey = (s: State, key: string) => s.tasks.find(t => t.key === key);
export const projectOf = (s: State, id: number) => s.projects.find(p => p.id === id);
