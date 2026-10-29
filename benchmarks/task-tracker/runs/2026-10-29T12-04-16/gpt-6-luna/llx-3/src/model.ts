export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; key: string; name: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string; updatedAt: string;
  version: number; deps: string[]; history: unknown[];
};
export type State = {
  users: User[]; projects: Project[]; members: Map<number, Map<number, string>>;
  tasks: Task[]; comments: Map<string, unknown[]>; tokens: Map<string, number>;
  limits: Map<string, { minute: number; count: number }>;
  idem: Map<string, { body: string; task: Task; at: number }>;
  sequences: Map<number, number>;
  now: () => number;
};
