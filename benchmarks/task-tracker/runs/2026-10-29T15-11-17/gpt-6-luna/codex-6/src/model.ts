export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string; next: number };
export type Task = { id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[]; assigneeId: number | null;
  createdBy: number; createdAt: string; updatedAt: string; version: number };
export type History = { at: string; by: number; field: string; from: unknown; to: unknown };
export type State = {
  users: User[]; projects: Project[]; members: Map<number, Map<number, string>>;
  tasks: Task[]; comments: Map<number, unknown[]>; history: Map<number, History[]>;
  deps: Map<number, Set<number>>; tokens: Map<string, number>;
  limits: Map<string, { minute: number; count: number }>;
  idem: Map<string, { at: number; signature: string; body: unknown }>;
  ids: { user: number; project: number; task: number; comment: number };
};
export function state(): State {
  return { users: [], projects: [], members: new Map(), tasks: [], comments: new Map(),
    history: new Map(), deps: new Map(), tokens: new Map(), limits: new Map(), idem: new Map(),
    ids: { user: 0, project: 0, task: 0, comment: 0 } };
}
export type Context = { s: State; now: () => number };
