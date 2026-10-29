export type User = { id: number; email: string; password: string; name: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; projectId: number; number: number; key: string; title: string;
  description: string | null; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string; updatedAt: string;
  version: number;
};
export type State = {
  users: User[]; projects: Project[]; tasks: Task[];
  memberships: Map<number, Map<number, string>>;
  sessions: Map<string, number>; histories: Map<number, object[]>;
  comments: Map<number, object[]>; deps: Map<number, Set<number>>;
  idempotency: Map<string, { at: number; body: string; result: Task }>;
  rate: Map<string, { minute: number; count: number }>;
  nextUser: number; nextProject: number; nextTask: number; nextComment: number;
  taskNumbers: Map<number, number>;
};
export type Context = { state: State; now: () => number };
export const iso = (ctx: Context) => new Date(ctx.now()).toISOString();
export function member(ctx: Context, projectId: number, userId: number): string | undefined {
  return ctx.state.memberships.get(projectId)?.get(userId);
}
