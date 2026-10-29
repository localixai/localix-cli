import {
  canWrite,
  err,
  fail,
  json,
  labels,
  role,
  stamp,
  taskByKey,
  type State,
  type Task,
} from "./model";
const fields = ["assigneeId", "description", "labels", "priority", "title"];
export function changeTask(
  s: State,
  method: string,
  path: string,
  uid: number,
  body: any,
  ifMatch: string | null,
) {
  const m = path.match(/^\/tasks\/([^/]+)(?:\/(transitions))?$/);
  if (!m) return undefined;
  const t = taskByKey(s, m[1]!);
  if (!t) return fail(404, "not_found", "Task not found");
  const p = s.projects.find((x) => x.id === t.projectId)!;
  const r = role(s, p, uid);
  if (method === "PATCH" && !m[2]) {
    if (!canWrite(r)) return fail(403, "forbidden", "Cannot change task");
    for (const k of Object.keys(body)) if (!fields.includes(k)) return err(k);
    if (ifMatch && ifMatch !== `"${t.version}"`)
      return fail(412, "precondition_failed", "Task changed");
    const changes: any = {};
    for (const k of fields) {
      if (!(k in body)) continue;
      let v = body[k];
      if (k === "title") {
        if (typeof v !== "string" || !v.trim() || v.trim().length > 200)
          return err(k);
        v = v.trim();
      }
      if (k === "description" && typeof v !== "string") return err(k);
      if (k === "priority" && !["low", "normal", "high"].includes(v))
        return err(k);
      if (k === "labels") {
        v = labels(v);
        if (!v) return err(k);
      }
      if (
        k === "assigneeId" &&
        v !== null &&
        (!Number.isInteger(v) || !role(s, p, v))
      )
        return err(k);
      changes[k] = v;
    }
    const changed = fields.filter(
      (k) => k in changes && !same((t as any)[k], changes[k]),
    );
    if (changed.length) {
      const at = stamp(s);
      for (const k of changed) {
        s.history
          .get(t.id)!
          .push({ at, by: uid, field: k, from: (t as any)[k], to: changes[k] });
        (t as any)[k] = changes[k];
      }
      t.version++;
      t.updatedAt = at;
    }
    return json(t, 200, { ETag: `"${t.version}"` });
  }
  if (method === "POST" && m[2]) {
    if (!canWrite(r)) return fail(403, "forbidden", "Cannot move task");
    if (!["open", "in_progress", "done"].includes(body.to)) return err("to");
    if (ifMatch && ifMatch !== `"${t.version}"`)
      return fail(412, "precondition_failed", "Task changed");
    const valid =
      (t.status === "open" && body.to === "in_progress") ||
      (t.status === "in_progress" && ["open", "done"].includes(body.to)) ||
      (t.status === "done" && body.to === "open");
    if (!valid) return fail(409, "invalid_transition", "Invalid transition");
    if (
      body.to === "done" &&
      [...(s.deps.get(t.id) ?? [])].some((id) =>
        s.tasks.some((x) => x.id === id && x.status !== "done"),
      )
    )
      return fail(409, "blocked", "Dependencies are not done");
    const from = t.status;
    const at = stamp(s);
    t.status = body.to;
    t.version++;
    t.updatedAt = at;
    s.history
      .get(t.id)!
      .push({ at, by: uid, field: "status", from, to: t.status });
    return json(t, 200, { ETag: `"${t.version}"` });
  }
  return undefined;
}
function same(a: any, b: any) {
  return JSON.stringify(a) === JSON.stringify(b);
}
