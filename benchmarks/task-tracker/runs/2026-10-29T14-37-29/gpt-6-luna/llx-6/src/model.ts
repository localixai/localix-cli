export type Options = { now?: () => number };
export type User = { id: number; email: string; password: string; name: string };
export type Project = {
  id: number; name: string; key: string; ownerId: number; createdAt: string;
  members: Map<number, string>;
};
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string;
  updatedAt: string; version: number; dependsOn: string[];
  history: unknown[]; comments: unknown[];
};
export type State = {
  now: () => number; users: User[]; projects: Project[]; tasks: Task[];
  sessions: Map<string, number>; limits: Map<string, { start: number; count: number }>;
  ids: { user: number; project: number; task: number; comment: number };
  idem: Map<string, { body: string; task: Task; time: number }>;
  sequences: Map<number, number>;
};
export function time(s: State) { return new Date(s.now()).toISOString(); }
export function error(code: string, message: string, status: number, field?: string) {
  return Response.json({ error: { code, message, ...(field ? { field } : {}) } }, { status });
}
export const ok = (body: unknown, status = 200, headers?: Record<string, string>) =>
  body === null ? new Response(null, { status, headers }) : Response.json(body, { status, headers });
