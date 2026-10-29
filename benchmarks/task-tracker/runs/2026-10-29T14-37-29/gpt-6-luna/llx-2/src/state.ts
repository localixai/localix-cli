export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string; updatedAt: string; version: number;
  deleted?: boolean;
};
export class State {
  users: User[] = [];
  projects: Project[] = [];
  tasks: Task[] = [];
  members = new Map<number, Map<number, string>>();
  sessions = new Map<string, number>();
  comments = new Map<number, any[]>();
  history = new Map<number, any[]>();
  dependencies = new Map<number, Set<number>>();
  idem = new Map<string, { body: string; task: Task; at: number }>();
  limits = new Map<string, { minute: number; count: number }>();
  ids = { user: 0, project: 0, task: 0, token: 0 };
  constructor(public now: () => number) {}
  iso() { return new Date(this.now()).toISOString(); }
}
