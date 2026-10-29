import {
  canWrite,
  err,
  fail,
  json,
  labels,
  projectAccess,
  role,
  stamp,
  type State,
} from "./model";
export function createTask(
  s: State,
  uid: number,
  id: number,
  body: any,
  bulk = false,
) {
  const p = projectAccess(s, uid, id);
  if (!p) return fail(404, "not_found", "Project not found");
  if (!canWrite(role(s, p, uid)))
    return fail(403, "forbidden", "Cannot create tasks");
  const prefix = bulk ? "tasks[0]." : "";
  const title = typeof body.title === "string" ? body.title.trim() : "";
  if (!title || title.length > 200) return err(prefix + "title");
  if (body.description !== undefined && typeof body.description !== "string")
    return err(prefix + "description");
  const priority = body.priority ?? "normal";
  if (!["low", "normal", "high"].includes(priority))
    return err(prefix + "priority");
  const labs = body.labels === undefined ? [] : labels(body.labels);
  if (!labs) return err(prefix + "labels");
  const assigneeId = body.assigneeId ?? null;
  if (
    assigneeId !== null &&
    (!Number.isInteger(assigneeId) || !role(s, p, assigneeId))
  )
    return err(prefix + "assigneeId");
  const n =
    Math.max(
      0,
      ...s.tasks.filter((t) => t.projectId === id).map((t) => t.number),
    ) + 1;
  const now = stamp(s);
  const t = {
    id: ++s.ids.task,
    key: `${p.key}-${n}`,
    number: n,
    projectId: id,
    title,
    description: body.description ?? "",
    status: "open",
    priority,
    labels: labs,
    assigneeId,
    createdBy: uid,
    createdAt: now,
    updatedAt: now,
    version: 1,
  };
  s.tasks.push(t);
  s.history.set(t.id, []);
  s.deps.set(t.id, new Set());
  return json(t, 201, { ETag: '"1"' });
}
