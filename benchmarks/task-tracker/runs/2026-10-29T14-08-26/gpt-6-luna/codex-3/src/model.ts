export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string; next: number };
export type Task = {
  id: number; projectId: number; key: string; number: number; title: string;
  description: string; priority: string; labels: string[]; assigneeId: number | null;
  status: string; createdBy: number; createdAt: string; updatedAt: string; version: number;
};
export type Member = { userId: number; role: string };
export type Comment = { id: number; authorId: number; body: string; createdAt: string };
export type History = { at: string; by: number; field: string; from: unknown; to: unknown };
export type State = {
  users: User[]; projects: Project[]; tasks: Task[];
  sessions: Map<string, number>; members: Map<number, Member[]>;
  comments: Map<number, Comment[]>; history: Map<number, History[]>;
  deps: Map<number, Set<number>>; idem: Map<string, { hash: string; body: unknown; at: number }>;
  limits: Map<string, { minute: number; count: number }>;
  ids: { user: number; project: number; task: number; comment: number };
};
export const state = (): State => ({
  users: [], projects: [], tasks: [], sessions: new Map(), members: new Map(),
  comments: new Map(), history: new Map(), deps: new Map(), idem: new Map(),
  limits: new Map(), ids: { user: 0, project: 0, task: 0, comment: 0 },
});
