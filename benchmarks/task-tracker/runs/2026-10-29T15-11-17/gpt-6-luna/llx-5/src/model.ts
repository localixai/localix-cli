export type User = { id: number; email: string; password: string; name: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string; next: number };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string; description: string;
  status: string; priority: string; labels: string[]; assigneeId: number | null; createdBy: number;
  createdAt: string; updatedAt: string; version: number; deleted: boolean; deps: string[];
};
export type Store = {
  users: User[]; projects: Project[]; members: Map<number, Map<number, string>>;
  tasks: Task[]; sessions: Map<string, number>; history: Map<number, any[]>;
  comments: Map<number, any[]>; limits: Map<string, { start: number; count: number }>;
  idem: Map<string, { hash: string; body: any; expires: number }>;
  now: () => number; uid: number; pid: number; tid: number;
};
export const iso = (s: Store) => new Date(s.now()).toISOString();
export const memberRole = (s: Store, p: Project, u: number) =>
  u === p.ownerId ? "owner" : s.members.get(p.id)?.get(u);
export const projectForTask = (s: Store, t: Task) => s.projects.find((p) => p.id === t.projectId)!;
export const taskByKey = (s: Store, key: string) => s.tasks.find((t) => t.key === key && !t.deleted);
