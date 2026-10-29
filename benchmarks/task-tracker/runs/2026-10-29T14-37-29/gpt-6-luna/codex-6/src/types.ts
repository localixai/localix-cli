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
};
export type State = {
  users: User[];
  sessions: Map<string, number>;
  projects: Project[];
  members: Map<number, Map<number, string>>;
  tasks: Task[];
  histories: Map<number, any[]>;
  comments: Map<number, any[]>;
  deps: Map<number, Set<number>>;
  idem: Map<string, any>;
  limits: Map<string, { minute: number; count: number }>;
  now: () => number;
  ids: Record<string, number>;
};
export type Ctx = {
  state: State;
  req: Request;
  user: User | null;
  body: any;
  url: URL;
};
export type Result = {
  status: number;
  body?: any;
  headers?: Record<string, string>;
};
