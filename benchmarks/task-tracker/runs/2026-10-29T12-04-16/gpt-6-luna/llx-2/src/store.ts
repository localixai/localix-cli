export type User = { id: number; email: string; name: string; password: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; key: string; number: number; projectId: number; title: string;
  description: string; status: string; priority: string; labels: string[];
  assigneeId: number | null; createdBy: number; createdAt: string; updatedAt: string;
  version: number; history: any[]; comments: any[]; deps: Set<string>;
};
export class Store {
  users: User[] = [];
  projects: Project[] = [];
  tasks: Task[] = [];
  tokens = new Map<string, number>();
  members = new Map<string, string>();
  seq = 0;
  projectNumbers = new Map<number, number>();
  limits = new Map<string, { minute: number; count: number }>();
  idem = new Map<string, { body: string; task: Task; time: number }>();
  constructor(public now: () => number) {}
  iso() { return new Date(this.now()).toISOString(); }
  role(pid: number, uid: number) {
    const p = this.projects.find(x => x.id === pid);
    if (!p) return undefined;
    return p.ownerId === uid ? "owner" : this.members.get(`${pid}:${uid}`);
  }
  task(key: string) { return this.tasks.find(t => t.key === key); }
}
