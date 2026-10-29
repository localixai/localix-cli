import type { Ctx, Task } from "./types";
import { json, missing, validation, visibleProject } from "./util";
import { createTask, validateTask } from "./task-core";
import { body, denied } from "./util";

export async function taskList(ctx: Ctx): Promise<Response | null> {
  const m = ctx.url.pathname.match(/^\/projects\/(\d+)\/(tasks|stats)(?:\/(bulk))?$/);
  if (!m) return null;
  const p = visibleProject(ctx, Number(m[1]));
  if (!p) return missing();
  if (m[2] === "stats") {
    if (ctx.request.method !== "GET") return null;
    const tasks = ctx.store.tasks.filter(t => t.projectId === p.id && !t.deleted);
    const buckets = (field: "status" | "priority", values: string[]) =>
      Object.fromEntries(values.map(v => [v, tasks.filter(t => t[field] === v).length]));
    const counts = new Map<number, number>();
    tasks.forEach(t => { if (t.assigneeId !== null) counts.set(t.assigneeId, (counts.get(t.assigneeId) ?? 0) + 1); });
    return json({ total: tasks.length, byStatus: buckets("status", ["open", "in_progress", "done"]),
      byPriority: buckets("priority", ["high", "normal", "low"]),
      byAssignee: [...counts].map(([userId, count]) => ({ userId, count }))
        .sort((a, b) => b.count - a.count || a.userId - b.userId) });
  }
  if (m[3] && ctx.request.method === "POST") return bulk(ctx, p);
  if (ctx.request.method === "POST") {
    const role = p.members.get(ctx.user!.id)?.role;
    if (role === "viewer") return denied();
    const b = await body(ctx.request);
    if (!b) return validation("body");
    const bad = validateTask(b, ctx, p);
    if (bad) return validation(bad);
    const idem = ctx.request.headers.get("Idempotency-Key");
    if (idem) {
      const key = `${ctx.user!.id}:${idem}`, signature = JSON.stringify(b);
      const saved = ctx.store.idempotency.get(key);
      if (saved && ctx.store.now() - saved.time < 86_400_000) {
        if (saved.body !== signature)
          return (await import("./util")).fail("idempotency_mismatch", "Idempotency key reused", 422);
        return json(saved.result, 201, { "Idempotent-Replayed": "true", ETag:
          `"${(saved.result as any).version}"` });
      }
      const result = createTask(ctx, p, b);
      const { taskReply } = await import("./util");
      const response = taskReply(result, 201);
      ctx.store.idempotency.set(key, { time: ctx.store.now(), body: signature, result:
        await response.clone().json() });
      return response;
    }
    return (await import("./util")).taskReply(createTask(ctx, p, b), 201);
  }
  if (ctx.request.method === "GET") return listing(ctx, p);
  return null;
}

async function bulk(ctx: Ctx, p: any): Promise<Response> {
  if (p.members.get(ctx.user!.id)?.role === "viewer") return denied();
  const b = await body(ctx.request);
  if (!Array.isArray(b?.tasks) || b.tasks.length < 1 || b.tasks.length > 50) return validation("tasks");
  for (let i = 0; i < b.tasks.length; i++) {
    const bad = validateTask(b.tasks[i], ctx, p, `tasks[${i}].`);
    if (bad) return validation(bad);
  }
  const items = b.tasks.map((x: any) => {
    createTask(ctx, p, x);
    const task = ctx.store.tasks.at(-1)!;
    return Object.fromEntries(Object.entries(task).filter(([k]) =>
      !["deleted", "dependencies"].includes(k)));
  });
  return json({ items }, 201);
}

function listing(ctx: Ctx, p: any) {
  const q = ctx.url.searchParams, sort = q.get("sort") ?? "created";
  const status = q.get("status"), assignee = q.get("assignee");
  let limit = q.has("limit") ? Number(q.get("limit")) : 20;
  if (status && !["open", "in_progress", "done"].includes(status)) return validation("status");
  if (!["created", "-created", "priority"].includes(sort)) return validation("sort");
  if (!Number.isInteger(limit) || limit < 1 || limit > 100) return validation("limit");
  if (assignee && !["me", "none"].includes(assignee) && !/^\d+$/.test(assignee)) return validation("assignee");
  const cursor = q.get("cursor");
  let offset = 0;
  if (cursor) {
    try {
      const data = JSON.parse(atob(cursor));
      if (data.project !== p.id || data.sort !== sort || data.status !== status ||
          data.assignee !== assignee || data.label !== q.get("label") || data.q !== q.get("q")) throw 0;
      offset = data.offset;
    } catch { return validation("cursor"); }
  }
  let tasks: Task[] = ctx.store.tasks.filter(t => t.projectId === p.id && !t.deleted);
  if (status) tasks = tasks.filter(t => t.status === status);
  if (assignee === "me") tasks = tasks.filter(t => t.assigneeId === ctx.user!.id);
  else if (assignee === "none") tasks = tasks.filter(t => t.assigneeId === null);
  else if (assignee) tasks = tasks.filter(t => t.assigneeId === Number(assignee));
  const label = q.get("label")?.toLowerCase();
  if (label) tasks = tasks.filter(t => t.labels.includes(label));
  const query = q.get("q")?.toLowerCase();
  if (query) tasks = tasks.filter(t => `${t.title} ${t.description}`.toLowerCase().includes(query));
  tasks.sort(sort === "-created" ? (a, b) => b.number - a.number :
    sort === "priority" ? (a, b) => ["high", "normal", "low"].indexOf(a.priority) -
      ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number : (a, b) => a.number - b.number);
  const page = tasks.slice(offset, offset + limit);
  const next = offset + limit < tasks.length ? btoa(JSON.stringify({
    project: p.id, sort, status, assignee, label: q.get("label"), q: q.get("q"), offset: offset + limit,
  })) : null;
  return json({ items: page, nextCursor: next });
}
