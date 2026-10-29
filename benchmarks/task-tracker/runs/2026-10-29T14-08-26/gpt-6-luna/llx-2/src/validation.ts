import { bad } from "./http";
import type { Project, Store, Task } from "./model";
export function taskFields(body: Record<string, any>, project: Project, store: Store) {
  const title = typeof body.title === "string" ? body.title.trim() : "";
  if (!title || title.length > 200) bad("title");
  const description = body.description ?? "";
  if (typeof description !== "string") bad("description");
  const priority = body.priority ?? "normal";
  if (!["low", "normal", "high"].includes(priority)) bad("priority");
  let labels = body.labels ?? [];
  if (!Array.isArray(labels) || labels.some((x: any) => typeof x !== "string" ||
    !x.trim() || x.trim().length > 30 || /\s/.test(x.trim()))) bad("labels");
  labels = [...new Set(labels.map((x: string) => x.trim().toLowerCase()))].sort();
  const assigneeId = body.assigneeId ?? null;
  if (assigneeId !== null && (!Number.isInteger(assigneeId) ||
    !project.members.has(assigneeId))) bad("assigneeId");
  return { title, description, priority, labels, assigneeId };
}
export function patchFields(body: Record<string, any>, task: Task, project: Project, store: Store) {
  const allowed = ["title", "description", "priority", "labels", "assigneeId"];
  for (const key of Object.keys(body)) if (!allowed.includes(key)) bad(key);
  if (!Object.keys(body).length) bad("body");
  const values = taskFields({ ...task, ...body }, project, store);
  return Object.fromEntries(Object.keys(body).map((key) => [key, values[key as keyof typeof values]]));
}
