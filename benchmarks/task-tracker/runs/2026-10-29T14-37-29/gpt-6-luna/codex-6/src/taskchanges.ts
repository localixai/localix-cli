import type { Ctx, Result, Task } from "./types";
import { cleanLabels, etag, fail, iso, member, ok } from "./util";
export function patchTask(c: Ctx, t: Task, b: any): Result {
  if (member(c, t.projectId) === "viewer") return fail(403, "forbidden");
  for (const k of Object.keys(b || {}))
    if (
      !["title", "description", "priority", "labels", "assigneeId"].includes(k)
    )
      return fail(400, "validation", "unknown field", k);
  const old = {
    title: t.title,
    description: t.description,
    priority: t.priority,
    labels: t.labels,
    assigneeId: t.assigneeId,
  };
  if (
    b.title !== undefined &&
    (typeof b.title !== "string" ||
      !b.title.trim() ||
      b.title.trim().length > 200)
  )
    return fail(400, "validation", "invalid title", "title");
  if (b.description !== undefined && typeof b.description !== "string")
    return fail(400, "validation", "invalid description", "description");
  if (
    b.priority !== undefined &&
    !["low", "normal", "high"].includes(b.priority)
  )
    return fail(400, "validation", "invalid priority", "priority");
  if (b.labels !== undefined && cleanLabels(b.labels) === null)
    return fail(400, "validation", "invalid labels", "labels");
  if (
    b.assigneeId !== undefined &&
    b.assigneeId !== null &&
    !c.state.members.get(t.projectId)?.has(b.assigneeId)
  )
    return fail(400, "validation", "invalid assignee", "assigneeId");
  const vals: any = { ...b };
  if (vals.title !== undefined) vals.title = vals.title.trim();
  if (vals.labels !== undefined) vals.labels = cleanLabels(vals.labels);
  const fields = Object.keys(vals).sort(),
    changed = fields.filter(
      (k) => JSON.stringify((old as any)[k]) !== JSON.stringify(vals[k]),
    );
  if (changed.length) {
    for (const k of changed) {
      const to = vals[k];
      c.state.histories
        .get(t.id)!
        .push({
          at: iso(c),
          by: c.user!.id,
          field: k,
          from: (old as any)[k],
          to,
        });
      (t as any)[k] = to;
    }
    t.version++;
    t.updatedAt = iso(c);
  }
  return ok(t, 200, etag(t));
}
export function transition(c: Ctx, t: Task, b: any): Result {
  if (member(c, t.projectId) === "viewer") return fail(403, "forbidden");
  if (!["open", "in_progress", "done"].includes(b?.to))
    return fail(400, "validation", "invalid status", "to");
  const valid =
    (t.status === "open" && b.to === "in_progress") ||
    (t.status === "in_progress" && ["open", "done"].includes(b.to)) ||
    (t.status === "done" && b.to === "open");
  if (!valid) return fail(409, "invalid_transition");
  if (b.to === "done")
    for (const id of c.state.deps.get(t.id) || []) {
      const d = c.state.tasks.find((x) => x.id === id);
      if (d && d.status !== "done") return fail(409, "blocked");
    }
  const from = t.status;
  t.status = b.to;
  t.version++;
  t.updatedAt = iso(c);
  c.state.histories
    .get(t.id)!
    .push({ at: iso(c), by: c.user!.id, field: "status", from, to: b.to });
  return ok(t, 200, etag(t));
}
export function comments(c: Ctx, t: Task, m: string, b: any): Result {
  const rows = c.state.comments.get(t.id)!;
  if (m === "GET") return ok(rows);
  if (typeof b?.body !== "string" || !b.body.trim() || b.body.length > 2000)
    return fail(400, "validation", "invalid body", "body");
  const item = {
    id: rows.length + 1,
    authorId: c.user!.id,
    body: b.body.trim(),
    createdAt: iso(c),
  };
  rows.push(item);
  return ok(item, 201);
}
