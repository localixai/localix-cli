export type User = {
  id: number;
  email: string;
  password: string;
  name: string;
};
export type Project = {
  id: number;
  key: string;
  name: string;
  ownerId: number;
  createdAt: string;
  members: Map<number, string>;
  seq: number;
};
export type Task = {
  id: number;
  key: string;
  number: number;
  projectId: number;
  title: string;
  description: string;
  status: string;
  priority: string;
  labels: string[];
  assigneeId: number | null;
  createdBy: number;
  createdAt: string;
  updatedAt: string;
  version: number;
  deps: Set<string>;
  history: any[];
  comments: any[];
};
export type State = {
  users: User[];
  projects: Project[];
  tasks: Task[];
  sessions: Map<string, number>;
  idem: Map<string, { fingerprint: string; task: Task; at: number }>;
  limits: Map<string, { minute: number; count: number }>;
  now: () => number;
  ids: { user: number; project: number; task: number; comment: number };
};
export function iso(s: State) {
  return new Date(s.now()).toISOString();
}
export function err(
  code: string,
  message = code,
  field?: string,
  status = 400,
) {
  return Response.json(
    { error: { code, message, ...(field ? { field } : {}) } },
    { status },
  );
}
export function ok(body: any, status = 200, headers?: Record<string, string>) {
  return Response.json(body, { status, headers });
}
export function projectFor(s: State, id: number, user: number) {
  return s.projects.find((p) => p.id === id && p.members.has(user));
}
export function byKey(s: State, key: string) {
  return s.tasks.find((t) => t.key === key);
}
export function taskVisible(s: State, key: string, user: number) {
  const t = byKey(s, key);
  return t && projectFor(s, t.projectId, user) ? t : undefined;
}
export function cleanLabels(x: any) {
  return [...new Set<string>(x.map((v: string) => v.trim().toLowerCase()))].sort();
}
export function validateTask(
  b: any,
  field = (x: string) => x,
): string | undefined {
  if (typeof b.title !== "string" || !b.title.trim() || b.title.length > 200)
    return field("title");
  if (b.description !== undefined && typeof b.description !== "string")
    return field("description");
  if (
    b.priority !== undefined &&
    !["low", "normal", "high"].includes(b.priority)
  )
    return field("priority");
  if (
    b.labels !== undefined &&
    (!Array.isArray(b.labels) ||
      b.labels.some(
        (x: any) =>
          typeof x !== "string" ||
          !x.trim() ||
          x.length > 30 ||
          /\s/.test(x.trim()),
      ))
  )
    return field("labels");
  return undefined;
}
export function makeTask(s: State, p: Project, u: number, b: any): Task {
  const n = ++p.seq,
    at = iso(s);
  const t: Task = {
    id: ++s.ids.task,
    key: `${p.key}-${n}`,
    number: n,
    projectId: p.id,
    title: b.title.trim(),
    description: b.description ?? "",
    status: "open",
    priority: b.priority ?? "normal",
    labels: b.labels ? cleanLabels(b.labels) : [],
    assigneeId: b.assigneeId ?? null,
    createdBy: u,
    createdAt: at,
    updatedAt: at,
    version: 1,
    deps: new Set(),
    history: [],
    comments: [],
  };
  s.tasks.push(t);
  return t;
}
