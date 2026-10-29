export type User = { id: number; email: string; name: string; password: string };
export type Member = { userId: number; role: "owner" | "member" | "viewer" };
export type Project = {
  id: number; name: string; key: string; ownerId: number; createdAt: string;
  members: Map<number, Member>; nextTask: number;
};
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: "open" | "in_progress" | "done";
  priority: "low" | "normal" | "high"; labels: string[]; assigneeId: number | null;
  createdBy: number; createdAt: string; updatedAt: string; version: number;
  deleted: boolean; dependencies: Set<string>;
};
export type Store = {
  users: User[]; projects: Project[]; tasks: Task[]; tokens: Map<string, number>;
  comments: Map<string, unknown[]>; history: Map<string, unknown[]>;
  now: () => number; nextToken: number; limits: Map<string, { minute: number; count: number }>;
  idempotency: Map<string, { time: number; body: string; result: unknown }>;
};
export type Ctx = { store: Store; user: User | null; request: Request; url: URL; params: string[] };
