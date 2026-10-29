export type User = { id: number; email: string; password: string; name: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string; description: string;
  status: string; priority: string; labels: string[]; assigneeId: number | null; createdBy: number;
  createdAt: string; updatedAt: string; version: number;
};
export type State = {
  users: User[]; projects: Project[]; tasks: Task[]; sessions: Map<string, number>;
  members: Map<number, Map<number, string>>; comments: Map<number, any[]>;
  history: Map<number, any[]>; deps: Map<number, Set<number>>; next: Record<string, number>;
};
export const initial = (): State => ({
  users: [], projects: [], tasks: [], sessions: new Map(), members: new Map(),
  comments: new Map(), history: new Map(), deps: new Map(),
  next: { user: 1, project: 1, task: 1, comment: 1 },
});
