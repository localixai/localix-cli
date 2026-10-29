export type User = {
  id: number;
  email: string;
  password: string;
  name: string;
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
  key: string;
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
  history: unknown[];
  comments: unknown[];
  deps: Set<string>;
};
export type State = {
  users: User[];
  projects: Project[];
  tasks: Task[];
  sessions: Map<string, number>;
  members: Map<number, Map<number, string>>;
  limits: Map<string, { minute: number; count: number }>;
  idem: Map<string, { body: string; task: unknown; at: number }>;
  now: () => number;
  ids: { user: number; project: number; task: number; comment: number };
};
export const iso = (s: State) => new Date(s.now()).toISOString();
export const membership = (s: State, p: Project, u: number) =>
  u === p.ownerId ? "owner" : s.members.get(p.id)?.get(u);
export function taskByKey(s: State, key: string) {
  return s.tasks.find((t) => t.key === key);
}
