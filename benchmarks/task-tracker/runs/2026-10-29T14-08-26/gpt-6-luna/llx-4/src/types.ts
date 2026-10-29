export type User = { id: number; email: string; password: string; name: string };
export type Project = {
  id: number; name: string; key: string; ownerId: number; createdAt: string;
  members: Map<number, string>; next: number;
};
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string; updatedAt: string;
  version: number; history: unknown[]; comments: unknown[]; dependsOn: Set<string>;
};
export type State = {
  users: User[]; projects: Project[]; tasks: Task[]; tokens: Map<string, number>;
  limits: Map<string, { minute: number; count: number }>;
  idem: Map<string, { time: number; body: string; result: unknown }>;
  now: () => number; ids: { user: number; project: number; task: number; comment: number };
};
