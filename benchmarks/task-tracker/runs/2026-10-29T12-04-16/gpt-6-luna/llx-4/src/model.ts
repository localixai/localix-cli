export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string; description: string;
  status: string; priority: string; labels: string[]; assigneeId: number | null;
  createdBy: number; createdAt: string; updatedAt: string; version: number;
};
export type Comment = { id: number; authorId: number; body: string; createdAt: string };
export type State = {
  users: User[]; projects: Project[]; tasks: Task[]; sessions: Map<string, number>;
  members: Map<number, Map<number, string>>; comments: Map<number, Comment[]>;
  history: Map<number, object[]>; deps: Map<number, Set<number>>;
  counts: Map<number, number>; limits: Map<string, { minute: number; used: number }>;
  idem: Map<string, { at: number; body: string; task: Task }>;
  ids: { user: number; project: number; task: number; comment: number };
};
export function state(): State {
  return {
    users: [], projects: [], tasks: [], sessions: new Map(), members: new Map(),
    comments: new Map(), history: new Map(), deps: new Map(), counts: new Map(),
    limits: new Map(), idem: new Map(),
    ids: { user: 0, project: 0, task: 0, comment: 0 },
  };
}
