export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; key: string; name: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string; updatedAt: string;
  version: number; history: any[]; comments: any[]; deps: Set<string>;
};
export type State = {
  users: User[]; projects: Project[]; tasks: Task[]; members: Map<number, Map<number, string>>;
  tokens: Map<string, number>; counters: Map<string, number>; limits: Map<string, number[]>;
  idem: Map<string, { at: number; body: string; task: Task }>;
  now: () => number; nextUser: number; nextProject: number; nextTask: number;
  nextComment: number; nextToken: number;
};
export const iso = (s: State) => new Date(s.now()).toISOString();
export const taskOut = (t: Task) => {
  const { history, comments, deps, assignee, ...out } = t as Task & { assignee?: number | null };
  return out;
};
