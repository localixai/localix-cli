import { iso, taskByKey, type Task } from "./model";
import {
  bad,
  canWrite,
  conflict,
  denied,
  empty,
  err,
  member,
  missing,
  ok,
  taskView,
  type Ctx,
} from "./http";
import { listing } from "./list";
import {
  comments,
  dependencies,
  deleteTask,
  patchTask,
  transition,
} from "./task-actions";

const priorities = ["low", "normal", "high"],
  statuses = ["open", "in_progress", "done"];
export function labels(v: any): string[] | null {
  if (!Array.isArray(v)) return null;
  const out: string[] = [];
  for (const x of v) {
    if (typeof x !== "string") return null;
    const z = x.trim().toLowerCase();
    if (!z || z.length > 30 || /\s/.test(z)) return null;
    out.push(z);
  }
  return [...new Set(out)].sort();
}
export function validate(b: any, partial = false): string | undefined {
  const allowed = partial
    ? ["title", "description", "priority", "labels", "assigneeId"]
    : ["title", "description", "priority", "labels", "assigneeId"];
  for (const k of Object.keys(b)) if (!allowed.includes(k)) return k;
  if (
    (!partial || "title" in b) &&
    (typeof b.title !== "string" ||
      !b.title.trim() ||
      b.title.trim().length > 200)
  )
    return "title";
  if ("description" in b && typeof b.description !== "string")
    return "description";
  if ("priority" in b && !priorities.includes(b.priority)) return "priority";
  if ("labels" in b && labels(b.labels) === null) return "labels";
  if (
    "assigneeId" in b &&
    b.assigneeId !== null &&
    !Number.isInteger(b.assigneeId)
  )
    return "assigneeId";
  return undefined;
}
export function assigned(c: Ctx, pid: number, id: any) {
  return (
    id === null ||
    id === undefined ||
    c.s.members.get(pid)?.has(id) ||
    c.s.projects.find((p) => p.id === pid)?.ownerId === id
  );
}
function create(c: Ctx, pid: number, b: any): Task | undefined {
  const p = c.s.projects.find((x) => x.id === pid)!;
  if (!assigned(c, pid, b.assigneeId)) return;
  const n = p.next++,
    now = iso(c.s),
    t: Task = {
      id: ++c.s.ids.task,
      key: `${p.key}-${n}`,
      number: n,
      projectId: pid,
      title: b.title.trim(),
      description: b.description ?? "",
      status: "open",
      priority: b.priority ?? "normal",
      labels: "labels" in b ? labels(b.labels)! : [],
      assigneeId: b.assigneeId ?? null,
      createdBy: c.user!,
      createdAt: now,
      updatedAt: now,
      version: 1,
      history: [],
      comments: [],
      deps: new Set(),
    };
  c.s.tasks.push(t);
  return t;
}
export function taskRoutes(c: Ctx, path: string, method: string) {
  const { s, b, user } = { s: c.s, b: c.body, user: c.user };
  const project = path.match(/^\/projects\/(\d+)\/tasks(?:\/bulk)?$/);
  if (project) {
    const pid = Number(project[1]),
      p = s.projects.find((x) => x.id === pid);
    if (!p || !member(c, pid)) return missing();
    if (method === "POST") {
      if (!canWrite(c, pid)) return denied();
      if (path.endsWith("/bulk")) return bulk(c, pid, b);
      const field = validate(b);
      if (field) return bad(field);
      if (!assigned(c, pid, b.assigneeId)) return bad("assigneeId");
      const ik = c.req.headers.get("Idempotency-Key"),
        key = ik ? `${user}:${ik}` : "",
        saved = key ? s.idem.get(key) : undefined;
      if (saved && c.s.now() - saved.at < 86400000) {
        if (saved.body !== JSON.stringify(b))
          return err(422, "idempotency_mismatch", "Idempotency key mismatch");
        return ok(saved.task, 201, {
          ETag: `"${(saved.task as any).version}"`,
          "Idempotent-Replayed": "true",
        });
      }
      const t = create(c, pid, b)!;
      const view = taskView(t);
      if (key)
        s.idem.set(key, { body: JSON.stringify(b), task: view, at: s.now() });
      return ok(view, 201, { ETag: '"1"' });
    }
    if (method === "GET") return listing(c, pid);
  }
  const match = path.match(
    /^\/tasks\/([^/]+)(?:\/(comments|history|transitions|dependencies)(?:\/([^/]+))?)?$/,
  );
  if (!match) return undefined;
  const key = decodeURIComponent(match[1]!),
    sub = match[2],
    t = taskByKey(s, key);
  if (!t || !member(c, t.projectId)) return missing();
  if (!sub) {
    if (method === "GET")
      return ok(taskView(t), 200, { ETag: `"${t.version}"` });
    if (method === "PATCH") return patchTask(c, t);
    if (method === "DELETE") return deleteTask(c, t);
  }
  if (sub === "comments") return comments(c, t, method);
  if (sub === "history" && method === "GET") return ok(t.history);
  if (sub === "transitions" && method === "POST") return transition(c, t);
  if (sub === "dependencies") return dependencies(c, t, method, path);
  return undefined;
}
function bulk(c: Ctx, pid: number, b: any) {
  if (!Array.isArray(b?.tasks) || b.tasks.length < 1 || b.tasks.length > 50)
    return bad("tasks");
  const errAt = b.tasks
    .map((x: any) => validate(x))
    .findIndex((x: string | undefined) => x);
  if (errAt >= 0) return bad(`tasks[${errAt}].${validate(b.tasks[errAt])}`);
  for (let i = 0; i < b.tasks.length; i++)
    if (!assigned(c, pid, b.tasks[i].assigneeId))
      return bad(`tasks[${i}].assigneeId`);
  return ok(
    { items: b.tasks.map((x: any) => taskView(create(c, pid, x)!)) },
    201,
  );
}
