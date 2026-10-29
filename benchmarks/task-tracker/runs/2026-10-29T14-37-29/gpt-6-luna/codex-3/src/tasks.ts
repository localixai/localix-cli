import {
  State,
  Task,
  Project,
  err,
  ok,
  iso,
  projectFor,
  taskVisible,
  validateTask,
  makeTask,
  cleanLabels,
} from "./model";
import { listTasks } from "./listing";
import { dependencies, comments, taskView } from "./task_aux";
export async function taskRoutes(
  s: State,
  r: Request,
  path: string,
  m: string,
  u: number | undefined,
): Promise<Response | undefined> {
  const b = async (): Promise<any | null> => {
    try {
      const x = await r.json();
      return x && typeof x === "object" && !Array.isArray(x) ? x : null;
    } catch {
      return null;
    }
  };
  const pm = path.match(/^\/projects\/(\d+)\/tasks(\/bulk)?$/);
  if (pm) {
    const p = projectFor(s, Number(pm[1]!), u ?? -1);
    if (!p) return err("not_found", "Not found", undefined, 404);
    if (m === "GET" && !pm[2]) return listTasks(s, r, p, u!);
    if (m === "POST") {
      if (p.members.get(u!) === "viewer")
        return err("forbidden", "Forbidden", undefined, 403);
      if (pm[2]) {
        const x = await b();
        if (
          !x ||
          !Array.isArray(x.tasks) ||
          x.tasks.length < 1 ||
          x.tasks.length > 50
        )
          return err("validation", "Invalid tasks", "tasks");
        for (let i = 0; i < x.tasks.length; i++) {
          const f = validateTask(x.tasks[i], (n) => `tasks[${i}].${n}`);
          if (f) return err("validation", "Invalid task", f);
        }
        return ok(
          { items: x.tasks.map((t: any) => taskView(makeTask(s, p, u!, t))) },
          201,
        );
      }
      const x = await b();
      if (!x) return err("validation", "Expected object", "body");
      const f = validateTask(x);
      if (f) return err("validation", "Invalid task", f);
      const a = assignee(s, p, x.assigneeId);
      if (a) return err("validation", "Invalid assignee", a);
      const idem = r.headers.get("Idempotency-Key");
      if (idem) {
        const k = `${u}:${idem}`,
          fingerprint = JSON.stringify(x),
          old = s.idem.get(k);
        if (old && s.now() - old.at < 86400000) {
          if (old.fingerprint !== fingerprint)
            return err("idempotency_mismatch", "Key reused", undefined, 422);
          return ok(taskView(old.task), 201, {
            ETag: '"' + old.task.version + '"',
            "Idempotent-Replayed": "true",
          });
        }
        const t = makeTask(s, p, u!, x);
        s.idem.set(k, { fingerprint, task: t, at: s.now() });
        return ok(taskView(t), 201, { ETag: '"1"' });
      }
      return ok(taskView(makeTask(s, p, u!, x)), 201, { ETag: '"1"' });
    }
    return undefined;
  }
  const mt = path.match(/^\/tasks\/([^/]+)(?:\/(.*))?$/);
  if (!mt) return undefined;
  const t = taskVisible(s, mt[1]!, u ?? -1);
  if (!t) return err("not_found", "Not found", undefined, 404);
  const role = s.projects.find((p) => p.id === t.projectId)!.members.get(u!);
  const sub = mt[2] || "";
  if (sub.startsWith("dependencies")) return dependencies(s, r, m, t, u!, role!);
  if (sub === "comments") return comments(s, r, m, t, u!);
  if (sub === "history" && m === "GET") return ok(t.history);
  if (!sub && m === "GET") return ok(taskView(t), 200, { ETag: `"${t.version}"` });
  if (!sub && m === "DELETE") {
    if (role === "viewer") return err("forbidden", "Forbidden", undefined, 403);
    if (role !== "owner" && t.createdBy !== u)
      return err("forbidden", "Forbidden", undefined, 403);
    remove(s, t);
    return new Response(null, { status: 204 });
  }
  if (!sub && m === "PATCH") {
    if (role === "viewer") return err("forbidden", "Forbidden", undefined, 403);
    const x = await b();
    if (!x) return err("validation", "Expected object", "body");
    for (const k of Object.keys(x))
      if (
        !["title", "description", "priority", "labels", "assigneeId"].includes(
          k,
        )
      )
        return err("validation", "Unknown field", k);
    const f = validateTask({ ...t, ...x });
    if (f) return err("validation", "Invalid task", f);
    const a = assignee(
      s,
      s.projects.find((p) => p.id === t.projectId)!,
      x.assigneeId,
    );
    if (a) return err("validation", "Invalid assignee", a);
    if (!precondition(r, t))
      return err("precondition_failed", "Task changed", undefined, 412);
    let changed = false;
    for (const k of Object.keys(x).sort()) {
      const v =
        k === "labels" ? cleanLabels(x[k]) : k === "title" ? x[k].trim() : x[k];
      if (JSON.stringify((t as any)[k]) !== JSON.stringify(v)) {
        t.history.push({
          at: iso(s),
          by: u,
          field: k,
          from: (t as any)[k],
          to: v,
        });
        (t as any)[k] = v;
        changed = true;
      }
    }
    if (changed) touch(s, t);
    return ok(taskView(t), 200, { ETag: `"${t.version}"` });
  }
  if (sub === "transitions" && m === "POST") {
    if (role === "viewer") return err("forbidden", "Forbidden", undefined, 403);
    const x = await b();
    if (!x) return err("validation", "Expected object", "body");
    if (!["open", "in_progress", "done"].includes(x.to))
      return err("validation", "Invalid status", "to");
    if (!precondition(r, t))
      return err("precondition_failed", "Task changed", undefined, 412);
    const valid =
      (t.status === "open" && x.to === "in_progress") ||
      (t.status === "in_progress" && ["open", "done"].includes(x.to)) ||
      (t.status === "done" && x.to === "open");
    if (!valid)
      return err("invalid_transition", "Invalid transition", undefined, 409);
    if (
      x.to === "done" &&
      [...t.deps].some((k) =>
        s.tasks.some((z) => z.key === k && z.status !== "done"),
      )
    )
      return err("blocked", "Dependencies incomplete", undefined, 409);
    const from = t.status;
    t.status = x.to;
    t.history.push({ at: iso(s), by: u, field: "status", from, to: x.to });
    touch(s, t);
    return ok(taskView(t), 200, { ETag: `"${t.version}"` });
  }
  return undefined;
}
function assignee(s: State, p: Project, id: any) {
  return id !== undefined &&
    id !== null &&
    (!Number.isInteger(id) || !p.members.has(id))
    ? "assigneeId"
    : undefined;
}
function precondition(r: Request, t: Task) {
  const v = r.headers.get("If-Match");
  return !v || v === `"${t.version}"`;
}
function touch(s: State, t: Task) {
  t.version++;
  t.updatedAt = iso(s);
}
function remove(s: State, t: Task) {
  for (const x of s.tasks) x.deps.delete(t.key);
  s.tasks = s.tasks.filter((x) => x !== t);
}
function byKey(s: State, k: string) {
  return s.tasks.find((t) => t.key === k);
}
