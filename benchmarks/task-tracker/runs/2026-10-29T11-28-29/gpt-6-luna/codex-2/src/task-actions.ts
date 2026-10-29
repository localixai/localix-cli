import { iso, taskByKey, type Task } from "./model";
import {
  bad,
  canWrite,
  conflict,
  denied,
  empty,
  err,
  missing,
  ok,
  taskView,
  type Ctx,
} from "./http";
import { assigned, labels, validate } from "./tasks";

const statuses = ["open", "in_progress", "done"];
export function patchTask(c: Ctx, t: Task) {
  if (!canWrite(c, t.projectId)) return denied();
  const b = c.body,
    field = validate(b, true);
  if (field) return bad(field);
  if (!assigned(c, t.projectId, b.assigneeId)) return bad("assigneeId");
  const pre = c.req.headers.get("If-Match");
  if (pre && pre !== `"${t.version}"`)
    return err(412, "precondition_failed", "Version changed");
  const changes: Record<string, [unknown, unknown]> = {};
  for (const k of Object.keys(b).sort()) {
    const value =
      k === "labels" ? labels(b[k]) : k === "title" ? b[k].trim() : b[k];
    if (JSON.stringify((t as any)[k]) !== JSON.stringify(value)) {
      changes[k] = [(t as any)[k], value];
      (t as any)[k] = value;
    }
  }
  if (Object.keys(changes).length) {
    t.version++;
    t.updatedAt = iso(c.s);
    for (const [field, [from, to]] of Object.entries(changes))
      t.history.push({ at: t.updatedAt, by: c.user, field, from, to });
  }
  return ok(taskView(t), 200, { ETag: `"${t.version}"` });
}
export function transition(c: Ctx, t: Task) {
  if (!canWrite(c, t.projectId)) return denied();
  const to = c.body?.to;
  if (!statuses.includes(to)) return bad("to");
  const pre = c.req.headers.get("If-Match");
  if (pre && pre !== `"${t.version}"`)
    return err(412, "precondition_failed", "Version changed");
  const valid =
    (t.status === "open" && to === "in_progress") ||
    (t.status === "in_progress" && ["open", "done"].includes(to)) ||
    (t.status === "done" && to === "open");
  if (!valid) return err(409, "invalid_transition", "Invalid transition");
  if (
    to === "done" &&
    [...t.deps].some((k) => taskByKey(c.s, k)?.status !== "done")
  )
    return err(409, "blocked", "Blocked");
  const from = t.status;
  t.status = to;
  t.version++;
  t.updatedAt = iso(c.s);
  t.history.push({ at: t.updatedAt, by: c.user, field: "status", from, to });
  return ok(taskView(t), 200, { ETag: `"${t.version}"` });
}
export function deleteTask(c: Ctx, t: Task) {
  if (!canWrite(c, t.projectId)) return denied();
  const role =
    c.s.projects.find((p) => p.id === t.projectId)?.ownerId === c.user;
  if (!role && t.createdBy !== c.user) return denied();
  for (const x of c.s.tasks) x.deps.delete(t.key);
  c.s.tasks.splice(c.s.tasks.indexOf(t), 1);
  return empty();
}
export function comments(c: Ctx, t: Task, method: string) {
  if (method === "GET") return ok(t.comments);
  if (method !== "POST") return undefined;
  const b = c.body;
  if (typeof b?.body !== "string" || !b.body.trim() || b.body.length > 2000)
    return bad("body");
  const v = {
    id: ++c.s.ids.comment,
    authorId: c.user,
    body: b.body.trim(),
    createdAt: iso(c.s),
  };
  t.comments.push(v);
  return ok(v, 201);
}
export function dependencies(c: Ctx, t: Task, method: string, path: string) {
  if (method === "GET") return ok(depView(c, t));
  if (method === "POST") {
    if (!canWrite(c, t.projectId)) return denied();
    const k = c.body?.dependsOn,
      d = taskByKey(c.s, k);
    if (!d || d.projectId !== t.projectId || d === t) return bad("dependsOn");
    if (t.deps.has(k)) return conflict();
    if (reaches(c, d.key, t.key))
      return err(409, "dependency_cycle", "Dependency cycle");
    t.deps.add(k);
    return ok(depView(c, t), 201);
  }
  const rm = path.match(/\/dependencies\/([^/]+)$/);
  if (rm && method === "DELETE") {
    if (!canWrite(c, t.projectId)) return denied();
    const k = decodeURIComponent(rm[1]!);
    if (!t.deps.delete(k)) return missing();
    return empty();
  }
  return undefined;
}
function depView(c: Ctx, t: Task) {
  const order = (a: string, b: string) =>
    taskByKey(c.s, a)!.number - taskByKey(c.s, b)!.number;
  return {
    key: t.key,
    dependsOn: [...t.deps].sort(order),
    blocks: c.s.tasks
      .filter((x) => x.deps.has(t.key))
      .map((x) => x.key)
      .sort(order),
  };
}
function reaches(
  c: Ctx,
  from: string,
  to: string,
  seen = new Set<string>(),
): boolean {
  if (from === to) return true;
  if (seen.has(from)) return false;
  seen.add(from);
  const task = taskByKey(c.s, from);
  return task ? [...task.deps].some((k) => reaches(c, k, to, seen)) : false;
}
