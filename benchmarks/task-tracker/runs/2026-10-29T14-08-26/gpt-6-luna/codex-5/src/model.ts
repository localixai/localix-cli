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
  members: Map<number, string>;
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
  deleted: boolean;
  deps: Set<string>;
  history: any[];
  comments: any[];
};
export type Store = {
  users: User[];
  projects: Project[];
  tasks: Task[];
  sessions: Map<string, number>;
  idem: Map<string, any>;
  counts: Map<string, { minute: number; count: number }>;
};
export const store = (): Store => ({
  users: [],
  projects: [],
  tasks: [],
  sessions: new Map(),
  idem: new Map(),
  counts: new Map(),
});
