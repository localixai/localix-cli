export type User = { id: number; email: string; password: string; name: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string; next: number };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string; description: string;
  status: string; priority: string; labels: string[]; assigneeId: number | null; createdBy: number;
  createdAt: string; updatedAt: string; version: number; deleted?: boolean;
};
export type History = { at: string; by: number; field: string; from: unknown; to: unknown };
export type State = {
  users: User[]; projects: Project[]; tasks: Task[]; tokens: Map<string, number>;
  members: Map<string, string>; comments: Map<string, any[]>; history: Map<number, History[]>;
  deps: Map<number, Set<number>>; rates: Map<string, { minute: number; count: number }>;
  idem: Map<string, { at: number; body: string; response: Task }>; id: number;
  now: () => number;
};
export const pair = (p: number, u: number) => `${p}:${u}`;
export const visible = (s: State, t: Task) => !t.deleted;
export function taskKey(s: State, key: string) {
  return s.tasks.find(t => t.key === key && visible(s, t));
}
