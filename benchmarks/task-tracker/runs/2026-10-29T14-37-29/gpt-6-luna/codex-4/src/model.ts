export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; number: number; projectId: number; title: string; description: string;
  status: string; priority: string; labels: string[]; assigneeId: number | null;
  createdBy: number; createdAt: string; updatedAt: string; version: number;
};
export type State = {
  users: User[]; projects: Project[]; tasks: Task[]; sessions: Map<string, number>;
  members: Map<number, Map<number, string>>; histories: Map<number, object[]>;
  comments: Map<number, object[]>; deps: Map<number, Set<number>>;
  ids: Record<string, number>; idem: Map<string, { at: number; body: string; task: Task }>;
  rates: Map<string, { minute: number; count: number }>;
};
export type Ctx = { s: State; now: () => number };
export function createState(): State {
  return { users: [], projects: [], tasks: [], sessions: new Map(), members: new Map(),
    histories: new Map(), comments: new Map(), deps: new Map(), ids: {}, idem: new Map(), rates: new Map() };
}
export const iso = (c: Ctx) => new Date(c.now()).toISOString();
export function next(c: Ctx, key: string) { c.s.ids[key] = (c.s.ids[key] || 0) + 1; return c.s.ids[key]; }
