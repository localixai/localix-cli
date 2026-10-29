export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string; updatedAt: string; version: number;
};
export type State = {
  users: User[]; projects: Project[]; tasks: Task[]; sessions: Map<string, number>;
  members: Map<string, string>; comments: Map<string, any[]>; history: Map<string, any[]>;
  deps: Map<number, Set<number>>; limits: Map<string, { minute: number; count: number }>;
  idem: Map<string, { at: number; body: string; task: Task }>;
};
export const pair = (a: number, b: number) => `${a}:${b}`;
export const memberRole = (s: State, p: Project, u: number) =>
  u === p.ownerId ? "owner" : s.members.get(pair(p.id, u));
export function createState(): State {
  return {
    users: [], projects: [], tasks: [], sessions: new Map(), members: new Map(),
    comments: new Map(), history: new Map(), deps: new Map(), limits: new Map(), idem: new Map(),
  };
}
