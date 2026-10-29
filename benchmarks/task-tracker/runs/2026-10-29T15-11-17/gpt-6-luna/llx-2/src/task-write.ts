import type { Ctx, Task } from "./types";
import { body, canWrite, conflict, denied, etag, fail, json, taskReply, validation } from "./util";
import { validateTask } from "./task-core";

export async function updateTask(ctx: Ctx, t: Task, transition = false): Promise<Response> {
  if (!canWrite(ctx, t)) return denied();
  if (ctx.request.headers.has("If-Match") && ctx.request.headers.get("If-Match") !== etag(t))
    return fail("precondition_failed", "Task version has changed", 412);
  const b = await body(ctx.request);
  if (!b) return validation("body");
  if (transition) {
    if (!["open", "in_progress", "done"].includes(b.to)) return validation("to");
    const allowed: Record<string, string[]> = { open: ["in_progress"], in_progress: ["open", "done"], done: ["open"] };
    if (!allowed[t.status]!.includes(b.to)) return fail("invalid_transition", "Invalid transition", 409);
    if (b.to === "done" && [...t.dependencies].some(key => ctx.store.tasks
      .some(x => x.key === key && !x.deleted && x.status !== "done")))
      return fail("blocked", "Task is blocked", 409);
    const from = t.status;
    t.status = b.to;
    t.version++;
    t.updatedAt = new Date(ctx.store.now()).toISOString();
    ctx.store.history.get(t.key)!.push({ at: t.updatedAt, by: ctx.user!.id, field: "status", from, to: t.status });
    return taskReply(t);
  }
  const allowed = ["title", "description", "priority", "labels", "assigneeId"];
  for (const key of Object.keys(b)) if (!allowed.includes(key)) return validation(key);
  const bad = validateTask({ ...t, ...b }, ctx,
    ctx.store.projects.find(p => p.id === t.projectId)!);
  if (bad) return validation(bad);
  const changed: string[] = [];
  for (const key of allowed.sort()) {
    if (!(key in b)) continue;
    let val: any = b[key];
    if (key === "title") val = val.trim();
    if (key === "labels") val = (await import("./util")).normalizeLabels(val);
    const before = (t as any)[key];
    if (JSON.stringify(before) !== JSON.stringify(val)) {
      (t as any)[key] = val;
      changed.push(key);
      ctx.store.history.get(t.key)!.push({ at: "", by: ctx.user!.id, field: key, from: before, to: val });
    }
  }
  if (changed.length) {
    t.version++;
    t.updatedAt = new Date(ctx.store.now()).toISOString();
    for (const h of ctx.store.history.get(t.key)!.slice(-changed.length)) (h as any).at = t.updatedAt;
  }
  return taskReply(t);
}
