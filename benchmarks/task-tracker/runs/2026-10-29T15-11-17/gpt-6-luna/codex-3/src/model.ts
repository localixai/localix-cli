export type User = { id: number; email: string; password: string; name: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string; next: number };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string; updatedAt: string;
  version: number; deleted: boolean;
};
export type History = { at: string; by: number; field: string; from: unknown; to: unknown };
export type State = {
  users: User[]; projects: Project[]; tasks: Task[]; tokens: Map<string, number>;
  members: Map<number, Map<number, string>>; comments: Map<number, any[]>;
  histories: Map<number, History[]>; deps: Map<number, Set<number>>;
  idem: Map<string, { at: number; body: string; reply: any }>;
  limits: Map<string, { minute: number; count: number }>;
  now: () => number; ids: { user: number; project: number; task: number; comment: number; token: number };
};
export const iso = (s: State) => new Date(s.now()).toISOString();
export const memberRole = (s: State, p: Project, u: number) =>
  u === p.ownerId ? "owner" : s.members.get(p.id)?.get(u);
export const visibleProject = (s: State, p: Project, u: number) => !!memberRole(s, p, u);
export function taskByKey(s: State, key: string) {
  return s.tasks.find((t) => t.key === key && !t.deleted);
}
