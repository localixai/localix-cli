import { bodyOf, fail, invalid, response } from "./errors";
import { iso, member, type Context, type Task } from "./model";
import { requireUser } from "./auth";
import { extraRoute } from "./task-extra";
import { list, stats } from "./task-list";

const priorities = ["high", "normal", "low"];
function labels(value: unknown, field = "labels"): string[] {
  if (!Array.isArray(value) || value.some((x) => typeof x !== "string" ||
    !x.trim() || x.trim().length > 30 || /\s/.test(x.trim()))) invalid(field);
  return [...new Set((value as string[]).map((x) => x.trim().toLowerCase()))].sort();
}
function validate(ctx: Context, projectId: number, b: any, prefix = "") {
  if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200) invalid(prefix + "title");
  if (b.description !== undefined && b.description !== null &&
    typeof b.description !== "string") invalid(prefix + "description");
  if (b.priority !== undefined && !priorities.includes(b.priority)) invalid(prefix + "priority");
  const ls = b.labels === undefined ? [] : labels(b.labels, prefix + "labels");
  if (b.assigneeId !== undefined && b.assigneeId !== null &&
    !ctx.state.memberships.get(projectId)?.has(b.assigneeId)) invalid(prefix + "assigneeId");
  return { title: b.title.trim(), description: b.description ?? "",
    priority: b.priority ?? "normal", labels: ls, assigneeId: b.assigneeId ?? null };
}
function lookup(ctx: Context, key: string) {
  const task = ctx.state.tasks.find((t) => t.key === key);
  const p = task && ctx.state.projects.find((x) => x.id === task.projectId);
  if (!task || !p) fail(404, "not_found", "Task not found");
  return { task, project: p };
}
function access(ctx: Context, req: Request, key: string, write = false) {
  const user = requireUser(ctx, req);
  const found = lookup(ctx, key);
  const role = member(ctx, found.project.id, user.id);
  if (!role) fail(404, "not_found", "Task not found");
  if (write && role === "viewer") fail(403, "forbidden", "Write access required");
  return { ...found, user };
}
function create(ctx: Context, projectId: number, data: ReturnType<typeof validate>, userId: number): Task {
  const project = ctx.state.projects.find((p) => p.id === projectId)!;
  const number = (ctx.state.taskNumbers.get(projectId) ?? 0) + 1;
  ctx.state.taskNumbers.set(projectId, number);
  const at = iso(ctx);
  const task: Task = { id: ctx.state.nextTask++, projectId, number, key: `${project.key}-${number}`,
    ...data, status: "open", createdBy: userId, createdAt: at, updatedAt: at, version: 1 };
  ctx.state.tasks.push(task);
  ctx.state.histories.set(task.id, []);
  ctx.state.deps.set(task.id, new Set());
  return task;
}
function etag(task: Task) { return { ETag: `"${task.version}"` }; }
function precondition(req: Request, task: Task) {
  const match = req.headers.get("If-Match");
  if (match && match !== `"${task.version}"`) fail(412, "precondition_failed", "Task has changed");
}

export async function taskRoute(ctx: Context, req: Request, url: URL, method: string) {
  const path = url.pathname;
  const statsMatch = path.match(/^\/projects\/(\d+)\/stats$/);
  if (statsMatch && method === "GET") {
    const user = requireUser(ctx, req);
    const id = Number(statsMatch[1]);
    if (!ctx.state.projects.some((p) => p.id === id && member(ctx, id, user.id))) {
      fail(404, "not_found", "Project not found");
    }
    return stats(ctx, id);
  }
  const bulk = path.match(/^\/projects\/(\d+)\/tasks\/bulk$/);
  const projectTasks = path.match(/^\/projects\/(\d+)\/tasks$/);
  if (projectTasks || bulk) {
    const user = requireUser(ctx, req);
    const id = Number((projectTasks ?? bulk)![1]);
    const p = ctx.state.projects.find((x) => x.id === id);
    const role = p && member(ctx, id, user.id);
    if (!p || !role) fail(404, "not_found", "Project not found");
    if (method === "GET" && projectTasks) return list(ctx, url, id, user.id);
    if (method === "GET" && path.endsWith("/stats")) return stats(ctx, id);
    if (method === "POST") {
      if (role === "viewer") fail(403, "forbidden", "Write access required");
      const b = await bodyOf(req);
      if (bulk) {
        if (!Array.isArray(b.tasks) || b.tasks.length < 1 || b.tasks.length > 50) invalid("tasks");
        const validated = b.tasks.map((x: any, i: number) => validate(ctx, id, x, `tasks[${i}].`));
        return response({ items: validated.map((x: any) => create(ctx, id, x, user.id)) }, 201);
      }
      const data = validate(ctx, id, b);
      const key = req.headers.get("Idempotency-Key");
      if (key) {
        const ik = `${user.id}:${key}`;
        const prev = ctx.state.idempotency.get(ik);
        if (prev && ctx.now() - prev.at < 86400000) {
          if (prev.body !== JSON.stringify(b)) fail(422, "idempotency_mismatch", "Key used with another body");
          return response(prev.result, 201, { ...etag(prev.result), "Idempotent-Replayed": "true" });
        }
        const result = create(ctx, id, data, user.id);
        ctx.state.idempotency.set(ik, { at: ctx.now(), body: JSON.stringify(b), result });
        return response(result, 201, etag(result));
      }
      const result = create(ctx, id, data, user.id);
      return response(result, 201, etag(result));
    }
  }
  const match = path.match(/^\/tasks\/([^/]+)(?:\/(.*))?$/);
  if (!match) return null;
  const key = decodeURIComponent(match[1]!);
  const sub = match[2] ?? "";
  if (sub === "") return single(ctx, req, key, method);
  if (sub === "transitions") return transition(ctx, req, key, method);
  if (sub === "history" && method === "GET") {
    const { task } = access(ctx, req, key);
    return response(ctx.state.histories.get(task.id));
  }
  if (sub.startsWith("dependencies") || sub === "comments") {
    return extraRoute(ctx, req, key, sub, method);
  }
  return null;
}

async function single(ctx: Context, req: Request, key: string, method: string) {
  if (method === "GET") {
    const { task } = access(ctx, req, key);
    return response(task, 200, etag(task));
  }
  if (method === "DELETE") {
    const { task, project, user } = access(ctx, req, key, true);
    if (member(ctx, project.id, user.id) !== "owner" && task.createdBy !== user.id) {
      fail(403, "forbidden", "Can only delete your own tasks");
    }
    ctx.state.tasks = ctx.state.tasks.filter((t) => t.id !== task.id);
    for (const deps of ctx.state.deps.values()) deps.delete(task.id);
    ctx.state.deps.delete(task.id);
    return response(null, 204);
  }
  if (method !== "PATCH") return null;
  const { task, project, user } = access(ctx, req, key, true);
  precondition(req, task);
  const b = await bodyOf(req);
  const allowed = ["title", "description", "priority", "labels", "assigneeId"];
  for (const field of Object.keys(b)) if (!allowed.includes(field)) invalid(field);
  const data = validate(ctx, project.id, { ...task, ...b });
  const changes = Object.entries(data).filter(([field, value]) =>
    JSON.stringify(task[field as keyof Task]) !== JSON.stringify(value));
  if (changes.length) {
    const at = iso(ctx);
    for (const [field, to] of changes.sort(([a], [b]) => a.localeCompare(b))) {
      ctx.state.histories.get(task.id)!.push({ at, by: user.id, field,
        from: task[field as keyof Task], to });
      (task as any)[field] = to;
    }
    task.version++;
    task.updatedAt = at;
  }
  return response(task, 200, etag(task));
}
async function transition(ctx: Context, req: Request, key: string, method: string) {
  if (method !== "POST") return null;
  const { task, user } = access(ctx, req, key, true);
  precondition(req, task);
  const b = await bodyOf(req);
  if (!["open", "in_progress", "done"].includes(b.to)) invalid("to");
  const valid = task.status === "open" ? b.to === "in_progress" :
    task.status === "in_progress" ? ["open", "done"].includes(b.to) : b.to === "open";
  if (!valid) fail(409, "invalid_transition", "Invalid status transition");
  if (b.to === "done") {
    const deps = ctx.state.deps.get(task.id) ?? new Set();
    if ([...deps].some((id) => ctx.state.tasks.find((t) => t.id === id)?.status !== "done")) {
      fail(409, "blocked", "Dependencies are not done");
    }
  }
  const at = iso(ctx);
  ctx.state.histories.get(task.id)!.push({ at, by: user.id, field: "status", from: task.status, to: b.to });
  task.status = b.to;
  task.version++;
  task.updatedAt = at;
  return response(task, 200, etag(task));
}
