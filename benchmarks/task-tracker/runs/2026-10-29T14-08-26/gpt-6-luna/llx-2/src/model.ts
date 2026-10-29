export type User = { id: number; email: string; name: string; password: string };
export type Project = {
  id: number; name: string; key: string; ownerId: number; createdAt: string;
  members: Map<number, string>; next: number;
};
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string; updatedAt: string;
  version: number; history: any[]; comments: any[]; depends: Set<string>;
};
export type Store = {
  users: User[]; projects: Project[]; tasks: Task[]; sessions: Map<string, number>;
  limits: Map<string, { minute: number; count: number }>;
  idem: Map<string, { at: number; signature: string; result: any }>;
  now: () => number; tokenSeq: number;
};
export const iso = (s: Store) => new Date(s.now()).toISOString();
