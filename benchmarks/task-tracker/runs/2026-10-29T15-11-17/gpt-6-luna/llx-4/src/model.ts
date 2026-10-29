export type User = { id: number; email: string; password: string; name: string };
export type Project = {
  id: number; name: string; key: string; ownerId: number; createdAt: string;
  members: Map<number, string>; next: number;
};
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string;
  updatedAt: string; version: number; deleted?: boolean;
};
export type Store = {
  now: () => number; users: User[]; projects: Project[]; tasks: Task[];
  tokens: Map<string, number>; comments: Map<number, any[]>; history: Map<number, any[]>;
  limits: Map<string, { minute: number; count: number }>;
  idempotency: Map<string, { at: number; body: string; result: Task }>;
  nextComment: number;
};
export const stamp = (s: Store) => new Date(s.now()).toISOString();
export const visible = (t: Task) => {
  const { deleted, ...task } = t;
  void deleted;
  return task;
};
