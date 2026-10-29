export type User = { id: number; email: string; password: string; name: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string;
  updatedAt: string; version: number; history: any[]; comments: any[];
  depends: Set<number>;
};
export type State = {
  users: User[]; projects: Project[]; tasks: Task[]; tokens: Map<string, number>;
  members: Map<number, Map<number, string>>; limits: Map<string, { minute: number; count: number }>;
  idem: Map<string, { body: string; result: any; at: number }>;
  ids: number; userIds: number; projectIds: number; commentIds: number; taskIds: number;
  sequences: Map<number, number>;
};
export const state = (): State => ({
  users: [], projects: [], tasks: [], tokens: new Map(), members: new Map(),
  limits: new Map(), idem: new Map(), ids: 0, userIds: 0, projectIds: 0,
  commentIds: 0, taskIds: 0, sequences: new Map(),
});
