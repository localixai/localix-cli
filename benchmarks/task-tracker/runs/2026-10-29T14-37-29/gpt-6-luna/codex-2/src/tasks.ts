import {
  Context,
  fail,
  json,
  nextId,
  iso,
  memberRole,
  visibleProject,
  taskByKey,
  Task,
} from "./model";
import { createTask, listTasks, change } from "./task-core";
import { dependencyRoutes } from "./dependencies";
export function taskRoutes(
  c: Context,
  path: string,
  method: string,
  body: any,
  url: URL,
) {
  const s = c.state,
    parts = path.split("/");
  if (
    parts[1] === "projects" &&
    ["tasks", "stats"].includes(parts[3] ?? "") &&
    /^\d+$/.test(parts[2] ?? "")
  ) {
    const p = visibleProject(c, Number(parts[2]));
    if (!p) return fail("not_found", "not found", 404);
    const role = memberRole(s, p.id, c.userId!);
    if (
      (parts[3] === "tasks" && parts[4] === "bulk" && method === "POST") ||
      (parts[3] === "tasks" && parts.length === 4 && method === "POST")
    ) {
      if (role === "viewer")
        return fail("forbidden", "write access required", 403);
      if (parts[4] === "bulk") {
        if (
          !Array.isArray(body.tasks) ||
          body.tasks.length < 1 ||
          body.tasks.length > 50
        )
          return fail("validation", "invalid tasks", 400, "tasks");
        const check = body.tasks.map((x: any) => validateTask(x));
        const bad = check.findIndex((x: any) => x);
        if (bad >= 0)
          return fail(
            "validation",
            "invalid task",
            400,
            `tasks[${bad}].${check[bad]}`,
          );
        return json(
          { items: body.tasks.map((x: any) => createTask(c, p, x).body) },
          201,
        );
      }
      const idem = c.request.headers.get("Idempotency-Key");
      if (idem) {
        const k = `${c.userId}:${idem}`,
          old = s.idempotency.get(k),
          raw = JSON.stringify(body);
        if (old && c.now() - old.at < 86400000)
          return old.body === raw
            ? json(old.value, 201, {
                ETag: '"1"',
                "Idempotent-Replayed": "true",
              })
            : fail("idempotency_mismatch", "key used with another body", 422);
        const made = createTask(c, p, body);
        if (made.status === 201)
          s.idempotency.set(k, { at: c.now(), body: raw, value: made.body });
        return made;
      }
      return createTask(c, p, body);
    }
    if (parts.length === 4 && parts[3] === "tasks" && method === "GET")
      return listTasks(c, p, url);
    if (parts[3] === "stats" && method === "GET") {
      const ts = s.tasks.filter((t) => t.projectId === p.id),
        assignees = new Map<number, number>();
      for (const t of ts)
        if (t.assigneeId !== null)
          assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0) + 1);
      return json({
        total: ts.length,
        byStatus: {
          open: ts.filter((t) => t.status === "open").length,
          in_progress: ts.filter((t) => t.status === "in_progress").length,
          done: ts.filter((t) => t.status === "done").length,
        },
        byPriority: {
          high: ts.filter((t) => t.priority === "high").length,
          normal: ts.filter((t) => t.priority === "normal").length,
          low: ts.filter((t) => t.priority === "low").length,
        },
        byAssignee: [...assignees]
          .map(([userId, count]) => ({ userId, count }))
          .sort((a, b) => b.count - a.count || a.userId - b.userId),
      });
    }
  }
  const t = parts[1] === "tasks" ? taskByKey(s, parts[2] ?? "") : undefined;
  if (!t || !memberRole(s, t.projectId, c.userId!))
    return parts[1] === "tasks"
      ? fail("not_found", "not found", 404)
      : undefined;
  const role = memberRole(s, t.projectId, c.userId!);
  if (parts.length === 3 && method === "GET")
    return json(t, 200, { ETag: `"${t.version}"` });
  if (parts.length === 3 && method === "DELETE") {
    if (role === "viewer")
      return fail("forbidden", "write access required", 403);
    if (t.createdBy !== c.userId && role !== "owner")
      return fail("forbidden", "cannot delete task", 403);
    s.tasks = s.tasks.filter((x) => x !== t);
    for (const deps of s.deps.values()) deps.delete(t.id);
    s.deps.delete(t.id);
    return { status: 204 };
  }
  if (parts[3] === "history" && method === "GET")
    return json(s.histories.get(t.id) ?? []);
  if (parts[3] === "comments") {
    if (method === "GET") return json(s.comments.get(t.id) ?? []);
    if (method === "POST") {
      const text = body.body;
      if (typeof text !== "string" || !text.trim() || text.length > 2000)
        return fail("validation", "invalid body", 400, "body");
      const comment = {
        id: nextId(s, `comment-${t.id}`),
        authorId: c.userId,
        body: text,
        createdAt: iso(c),
      };
      s.comments.get(t.id)!.push(comment);
      return json(comment, 201);
    }
  }
  if (parts[3] === "dependencies")
    return dependencyRoutes(c, t, parts, method, body);
  if (role === "viewer") return fail("forbidden", "write access required", 403);
  if (parts.length === 3 && method === "PATCH")
    return change(c, t, body, c.request);
  if (parts[3] === "transitions" && method === "POST")
    return transition(c, t, body, c.request);
}
function validateTask(b: any) {
  return !b.title || typeof b.title !== "string" || !b.title.trim()
    ? "title"
    : b.priority && !["low", "normal", "high"].includes(b.priority)
      ? "priority"
      : null;
}
function transition(c: Context, t: Task, b: any, r: Request) {
  if (!["open", "in_progress", "done"].includes(b.to))
    return fail("validation", "invalid status", 400, "to");
  const tag = r.headers.get("If-Match");
  if (tag && tag !== `"${t.version}"`)
    return fail("precondition_failed", "task changed", 412);
  const valid =
    (t.status === "open" && b.to === "in_progress") ||
    (t.status === "in_progress" && ["open", "done"].includes(b.to)) ||
    (t.status === "done" && b.to === "open");
  if (!valid) return fail("invalid_transition", "invalid transition", 409);
  if (b.to === "done")
    for (const id of c.state.deps.get(t.id) ?? [])
      if (c.state.tasks.find((x) => x.id === id)?.status !== "done")
        return fail("blocked", "dependencies are incomplete", 409);
  c.state.histories.get(t.id)!.push({
    at: iso(c),
    by: c.userId,
    field: "status",
    from: t.status,
    to: b.to,
  });
  t.status = b.to;
  t.version++;
  t.updatedAt = iso(c);
  return json(t, 200, { ETag: `"${t.version}"` });
}
