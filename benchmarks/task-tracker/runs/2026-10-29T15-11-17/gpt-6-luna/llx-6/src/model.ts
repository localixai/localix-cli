export type User = { id: number; email: string; password: string; name: string };
export type Project = {
  id: number; name: string; key: string; ownerId: number; createdAt: string;
  members: Map<number, string>; next: number;
};
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string; updatedAt: string;
  version: number; deleted: boolean; history: any[]; comments: any[]; depends: Set<string>;
};
export type State = {
  now: () => number; users: User[]; projects: Project[]; tasks: Task[];
  sessions: Map<string, number>; limits: Map<string, { minute: number; count: number }>;
  ids: { user: number; project: number; task: number; comment: number };
  idem: Map<string, { at: number; body: string; task: Task }>;
};
export const iso = (s: State) => new Date(s.now()).toISOString();
export const taskView = (t: Task) => {
  const { deleted, history, comments, depends, ...view } = t;
  return view;
};
export const error = (code: string, message: string, field?: string) => ({
  error: field ? { code, message, field } : { code, message },
});
export const bad = (field: string) => error("validation", `Invalid ${field}`, field);
