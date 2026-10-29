export type User = { id: number; email: string; password: string; name: string };
export type Project = {
  id: number; name: string; key: string; ownerId: number; createdAt: string;
  next: number; members: Map<number, string>;
};
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string; updatedAt: string;
  version: number; comments: any[]; history: any[]; deps: Set<number>;
};
export type State = {
  users: User[]; projects: Project[]; tasks: Task[]; sessions: Map<string, number>;
  rates: Map<string, { minute: number; count: number }>;
  idem: Map<string, { at: number; hash: string; task: any }>;
  uid: number; pid: number; tid: number; cid: number;
};
export const newState = (): State => ({
  users: [], projects: [], tasks: [], sessions: new Map(), rates: new Map(),
  idem: new Map(), uid: 0, pid: 0, tid: 0, cid: 0,
});
export const publicUser = (u: User) => ({ id: u.id, email: u.email, name: u.name });
export const publicProject = (p: Project) => ({
  id: p.id, name: p.name, key: p.key, ownerId: p.ownerId, createdAt: p.createdAt,
});
export const publicTask = (t: Task) => ({
  id: t.id, key: t.key, number: t.number, projectId: t.projectId, title: t.title,
  description: t.description, status: t.status, priority: t.priority, labels: t.labels,
  assigneeId: t.assigneeId, createdBy: t.createdBy, createdAt: t.createdAt,
  updatedAt: t.updatedAt, version: t.version,
});
