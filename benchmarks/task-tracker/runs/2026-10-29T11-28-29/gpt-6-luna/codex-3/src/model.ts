export type User = { id: number; email: string; password: string; name: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = { id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[]; assigneeId: number | null;
  createdBy: number; createdAt: string; updatedAt: string; version: number; dependsOn: string[];
  comments: unknown[]; history: unknown[] };
export type State = { users: User[]; projects: Project[]; memberships: Map<number, Map<number, string>>;
  tasks: Task[]; tokens: Map<string, number>; idempotency: Map<string, { at: number; body: string; task: unknown }>;
  hits: Map<string, number[]>; nextUser: number; nextProject: number; nextTask: number;
  nextComment: number; nextNumbers: Map<number, number>; now: () => number };
export function createState(now: () => number): State {
  return { users: [], projects: [], memberships: new Map(), tasks: [], tokens: new Map(),
    idempotency: new Map(), hits: new Map(), nextUser: 1, nextProject: 1, nextTask: 1,
    nextComment: 1, nextNumbers: new Map(), now };
}
