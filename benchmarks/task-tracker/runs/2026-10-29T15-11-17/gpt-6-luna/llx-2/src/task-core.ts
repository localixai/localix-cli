import type { Ctx, Project, Task } from "./types";
import { body, normalizeLabels, stamp, validation } from "./util";

export function validateTask(b: Record<string, any>, ctx: Ctx, p: Project, prefix = ""): string | null {
  if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200) return `${prefix}title`;
  if (b.description !== undefined && typeof b.description !== "string") return `${prefix}description`;
  if (b.priority !== undefined && !["low", "normal", "high"].includes(b.priority)) return `${prefix}priority`;
  if (b.labels !== undefined && normalizeLabels(b.labels) === null) return `${prefix}labels`;
  if (b.assigneeId !== undefined && b.assigneeId !== null &&
      !p.members.has(b.assigneeId)) return `${prefix}assigneeId`;
  return null;
}
export function createTask(ctx: Ctx, p: Project, b: Record<string, any>): Task {
  const number = p.nextTask++;
  const t: Task = { id: ctx.store.tasks.length + 1, key: `${p.key}-${number}`, number,
    projectId: p.id, title: b.title.trim(), description: b.description ?? "", status: "open",
    priority: b.priority ?? "normal", labels: normalizeLabels(b.labels ?? [])!,
    assigneeId: b.assigneeId ?? null, createdBy: ctx.user!.id, createdAt: stamp(ctx),
    updatedAt: stamp(ctx), version: 1, deleted: false, dependencies: new Set() };
  ctx.store.tasks.push(t);
  ctx.store.history.set(t.key, []);
  return t;
}
export async function createOne(ctx: Ctx, p: Project, prefix = "") {
  const b = await body(ctx.request);
  if (!b) return validation(prefix ? `${prefix}body` : "body");
  const field = validateTask(b, ctx, p, prefix);
  return field ? validation(field) : createTask(ctx, p, b);
}
