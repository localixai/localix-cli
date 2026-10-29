import { fail, stamp, validation, plainTask } from "./http";
import type { Context, Project, Task, User } from "./model";
export function clean(body: Record<string, any>, c: Context, p: Project) {
  const title = typeof body.title === "string" ? body.title.trim() : "";
  if (!title || title.length > 200) validation("title");
  const description = body.description === undefined ? "" : body.description;
  if (typeof description !== "string") validation("description");
  const priority = body.priority ?? "normal";
  if (!["low", "normal", "high"].includes(priority)) validation("priority");
  const rawLabels = body.labels ?? [];
  if (!Array.isArray(rawLabels)) validation("labels");
  const labels: string[] = [];
  for (const raw of rawLabels) {
    if (typeof raw !== "string") validation("labels");
    const label = raw.trim().toLowerCase();
    if (!label || label.length > 30 || /\s/.test(label)) validation("labels");
    if (!labels.includes(label)) labels.push(label);
  }
  labels.sort();
  const assigneeId = body.assigneeId ?? null;
  if (assigneeId !== null && (!Number.isInteger(assigneeId) ||
    c.s.members.get(p.id)?.has(assigneeId) !== true)) validation("assigneeId");
  return { title, description, priority, labels, assigneeId };
}
export function createTask(c: Context, p: Project, user: User, values: ReturnType<typeof clean>) {
  const number = p.next++;
  const now = stamp(c.now);
  const task: Task = { id: ++c.s.ids.task, key: `${p.key}-${number}`, number,
    projectId: p.id, ...values, status: "open", createdBy: user.id,
    createdAt: now, updatedAt: now, version: 1 };
  c.s.tasks.push(task);
  c.s.comments.set(task.id, []); c.s.history.set(task.id, []); c.s.deps.set(task.id, new Set());
  return task;
}
export function byKey(c: Context, key: string, userId: number) {
  const t = c.s.tasks.find((x) => x.key === key);
  const p = t && c.s.projects.find((x) => x.id === t.projectId);
  if (!t || !p || !c.s.members.get(p.id)?.has(userId)) fail(404, "not_found", "Task not found");
  return { t, p: p! };
}
export function history(c: Context, t: Task, user: User, field: string, from: unknown, to: unknown) {
  c.s.history.get(t.id)!.push({ at: stamp(c.now), by: user.id, field, from, to });
}
export function update(c: Context, t: Task, user: User, body: Record<string, any>) {
  const allowed = ["title", "description", "priority", "labels", "assigneeId"];
  for (const key of Object.keys(body)) if (!allowed.includes(key)) validation(key);
  const next = clean({ ...t, ...body }, c, c.s.projects.find((p) => p.id === t.projectId)!);
  let changed = false;
  for (const key of [...allowed].sort()) {
    if (!(key in body)) continue;
    const from = (t as any)[key]; const to = (next as any)[key];
    if (JSON.stringify(from) !== JSON.stringify(to)) {
      (t as any)[key] = to; history(c, t, user, key, from, to); changed = true;
    }
  }
  if (changed) { t.version++; t.updatedAt = stamp(c.now); }
  return t;
}
export function checkVersion(request: Request, t: Task) {
  const match = request.headers.get("If-Match");
  if (match && match !== `"${t.version}"`) fail(412, "precondition_failed", "Task has changed");
}
export function transition(c: Context, t: Task, user: User, to: unknown) {
  if (!["open", "in_progress", "done"].includes(String(to))) validation("to");
  const valid = (t.status === "open" && to === "in_progress") ||
    (t.status === "in_progress" && (to === "open" || to === "done")) ||
    (t.status === "done" && to === "open");
  if (!valid) fail(409, "invalid_transition", "Invalid status transition");
  if (to === "done" && [...c.s.deps.get(t.id)!].some((id) =>
    c.s.tasks.find((x) => x.id === id)?.status !== "done")) fail(409, "blocked", "Task has unfinished dependencies");
  const from = t.status; t.status = String(to); t.version++; t.updatedAt = stamp(c.now);
  history(c, t, user, "status", from, to);
}
export { plainTask };
