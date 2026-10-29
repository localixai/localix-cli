export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string;
  updatedAt: string; version: number; deps: string[];
};
export type State = {
  users: User[]; projects: Project[]; tasks: Task[]; members: Map<number, Map<number, string>>;
  sessions: Map<string, number>; comments: Map<string, any[]>; history: Map<string, any[]>;
  idem: Map<string, { at: number; body: string; task: Task }>;
  counts: Map<string, { minute: number; count: number }>;
  nextTask: Map<number, number>;
  now: () => number; ids: { user: number; project: number; task: number; token: number };
};
export const keyOf = (t: Task) => t.key;
export const iso = (s: State) => new Date(s.now()).toISOString();
