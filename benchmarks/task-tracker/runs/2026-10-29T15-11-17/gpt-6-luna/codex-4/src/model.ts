export type User = {
  id: number;
  email: string;
  name: string;
  password: string;
};
export type Project = {
  id: number;
  name: string;
  key: string;
  ownerId: number;
  createdAt: string;
  next: number;
};
export type Task = {
  id: number;
  number: number;
  projectId: number;
  title: string;
  description: string;
  status: string;
  priority: string;
  labels: string[];
  assigneeId: number | null;
  createdBy: number;
  createdAt: string;
  updatedAt: string;
  version: number;
};
export type State = {
  users: User[];
  projects: Project[];
  members: Map<string, string>;
  tokens: Map<string, number>;
  tasks: Task[];
  comments: Map<number, unknown[]>;
  history: Map<number, unknown[]>;
  deps: Map<number, Set<number>>;
  idems: Map<string, { at: number; body: string; result: unknown }>;
  counts: Map<string, { start: number; count: number }>;
};
export const pair = (a: number, b: number) => `${a}:${b}`;
export const publicUser = (u: User) => ({
  id: u.id,
  email: u.email,
  name: u.name,
});
export const projectView = (p: Project) => ({
  id: p.id,
  name: p.name,
  key: p.key,
  ownerId: p.ownerId,
  createdAt: p.createdAt,
});
export const taskView = (t: Task, p: Project) => ({
  id: t.id,
  key: `${p.key}-${t.number}`,
  number: t.number,
  projectId: t.projectId,
  title: t.title,
  description: t.description,
  status: t.status,
  priority: t.priority,
  labels: t.labels,
  assigneeId: t.assigneeId,
  createdBy: t.createdBy,
  createdAt: t.createdAt,
  updatedAt: t.updatedAt,
  version: t.version,
});
