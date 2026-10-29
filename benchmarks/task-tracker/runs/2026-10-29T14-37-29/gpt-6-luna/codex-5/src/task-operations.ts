import {
  body,
  err,
  iso,
  json,
  labels,
  role,
  validation,
  type Ctx,
  type Task,
} from "./model";
import { output } from "./task-data";
const priorities = ["low", "normal", "high"];
function canWrite(c: Ctx, t: Task) {
  return (
    role(
      c,
      c.db.projects.find((p) => p.id === t.projectId)!,
    ) !== "viewer"
  );
}
function versionCheck(c: Ctx, t: Task) {
  const h = c.req.headers.get("If-Match");
  return h && h !== `"${t.version}"` ? err("precondition_failed", 412) : null;
}
export async function patch(c: Ctx, t: Task) {
  if (!canWrite(c, t)) return err("forbidden", 403);
  const v = versionCheck(c, t);
  if (v) return v;
  const b = await body(c);
  if (!b) return validation();
  const allowed = ["title", "description", "priority", "labels", "assigneeId"];
  for (const k of Object.keys(b))
    if (!allowed.includes(k)) return validation(k);
  const changes: any = {};
  for (const k of allowed) {
    if (!(k in b)) continue;
    let val = b[k];
    if (
      k === "title" &&
      (typeof val !== "string" || !val.trim() || val.trim().length > 200)
    )
      return validation(k);
    if (k === "description" && typeof val !== "string") return validation(k);
    if (k === "priority" && !priorities.includes(val)) return validation(k);
    if (k === "labels") {
      val = labels(val);
      if (!val) return validation(k);
    }
    if (
      k === "assigneeId" &&
      val !== null &&
      !c.db.members.get(t.projectId)?.has(val)
    )
      return validation(k);
    changes[k] = val;
  }
  let changed = false;
  for (const k of Object.keys(changes).sort()) {
    const from = (t as any)[k],
      to = changes[k];
    if (JSON.stringify(from) === JSON.stringify(to)) continue;
    (t as any)[k] = to;
    t.history.push({ at: iso(c), by: c.user!.id, field: k, from, to });
    changed = true;
  }
  if (changed) {
    t.version++;
    t.updatedAt = iso(c);
  }
  return json(output(t), 200, { ETag: `"${t.version}"` });
}
export async function transition(c: Ctx, t: Task) {
  if (!canWrite(c, t)) return err("forbidden", 403);
  const v = versionCheck(c, t);
  if (v) return v;
  const b = await body(c);
  if (!b) return validation();
  if (!["open", "in_progress", "done"].includes(b.to)) return validation("to");
  const allowed =
    t.status === "open"
      ? b.to === "in_progress"
      : t.status === "in_progress"
        ? ["open", "done"].includes(b.to)
        : b.to === "open";
  if (!allowed) return err("invalid_transition", 409);
  if (
    b.to === "done" &&
    t.depends.some((id) =>
      c.db.tasks.some((x) => x.id === id && !x.deleted && x.status !== "done"),
    )
  )
    return err("blocked", 409);
  const from = t.status;
  t.status = b.to;
  t.version++;
  t.updatedAt = iso(c);
  t.history.push({
    at: iso(c),
    by: c.user!.id,
    field: "status",
    from,
    to: b.to,
  });
  return json(output(t), 200, { ETag: `"${t.version}"` });
}
