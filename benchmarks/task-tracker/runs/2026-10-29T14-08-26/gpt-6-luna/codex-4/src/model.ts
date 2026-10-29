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
};
export type Task = {
  id: number;
  projectId: number;
  key: string;
  number: number;
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
  history: object[];
  comments: object[];
  dependsOn: Set<number>;
};
export type State = {
  users: User[];
  projects: Project[];
  tasks: Task[];
  members: Map<number, Map<number, string>>;
  counters: Map<number, number>;
  sessions: Map<string, number>;
  idem: Map<string, { body: string; response: object; time: number }>;
  rates: Map<string, { minute: number; count: number }>;
  now: () => number;
};
export const cleanUser = (u: User) => ({
  id: u.id,
  email: u.email,
  name: u.name,
});
export const iso = (s: State) => new Date(s.now()).toISOString();
export function taskView(t: Task) {
  return {
    id: t.id,
    key: t.key,
    projectId: t.projectId,
    number: t.number,
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
  };
}
export function projectOf(s: State, id: number) {
  return s.projects.find((p) => p.id === id);
}
export function taskByKey(s: State, key: string) {
  return s.tasks.find((t) => t.key === key);
}
