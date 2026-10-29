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
  seq: number;
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
  deps: Set<string>;
};
export type State = {
  users: User[];
  projects: Project[];
  members: Map<number, Map<number, string>>;
  tasks: Task[];
  comments: any[];
  history: Map<string, any[]>;
  sessions: Map<string, number>;
  rates: Map<string, { minute: number; count: number }>;
  idem: Map<string, { at: number; body: string; reply: any }>;
  nextUser: number;
  nextProject: number;
  nextTask: number;
  nextComment: number;
};
export function state(): State {
  return {
    users: [],
    projects: [],
    members: new Map(),
    tasks: [],
    comments: [],
    history: new Map(),
    sessions: new Map(),
    rates: new Map(),
    idem: new Map(),
    nextUser: 1,
    nextProject: 1,
    nextTask: 1,
    nextComment: 1,
  };
}
export const taskView = (t: Task) => {
  const { deps, ...out } = t;
  return out;
};
export const pubUser = (u: User) => ({
  id: u.id,
  email: u.email,
  name: u.name,
});
export const stamp = (now: () => number) => new Date(now()).toISOString();
