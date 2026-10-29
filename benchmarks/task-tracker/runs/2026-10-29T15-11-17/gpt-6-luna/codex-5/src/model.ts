export type User = { id: number; email: string; password: string; name: string };
export type Project = { id: number; name: string; key: string; ownerId: number; createdAt: string };
export type Task = {
  id: number; number: number; projectId: number; title: string; description: string;
  status: string; priority: string; labels: string[]; assigneeId: number | null;
  createdBy: number; createdAt: string; updatedAt: string; version: number;
};
export type State = {
  users: User[]; tokens: Map<string, number>; projects: Project[];
  members: Map<number, Map<number, string>>; tasks: Task[];
  histories: Map<string, any[]>; comments: Map<string, any[]>;
  dependencies: Map<string, Set<string>>; counters: Record<string, number>;
  taskNumbers: Map<number, number>;
  idem: Map<string, { fingerprint: string; result: any; at: number }>;
  limits: Map<string, { minute: number; count: number }>;
};
export function keyOf(task: Task): string {
  return `${projectKey(task.projectId)}-${task.number}`;
}
let projectLookup: (id: number) => string = () => "";
export function setProjectLookup(fn: (id: number) => string) { projectLookup = fn; }
function projectKey(id: number) { return projectLookup(id); }
export function taskKey(state: State, task: Task) {
  const project = state.projects.find((p) => p.id === task.projectId);
  return `${project?.key}-${task.number}`;
}
export function fresh(): State {
  return {
    users: [], tokens: new Map(), projects: [], members: new Map(), tasks: [],
    histories: new Map(), comments: new Map(), dependencies: new Map(),
    counters: { user: 0, project: 0, task: 0, comment: 0 }, taskNumbers: new Map(), idem: new Map(),
    limits: new Map(),
  };
}
