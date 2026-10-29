import type { Project, State, Task } from "./model";
import { iso } from "./model";
import { bad } from "./http";
export function validateFields(b: any, s: State, projectId: number, prefix = "", patch = false) {
  if (!b || typeof b !== "object" || Array.isArray(b)) bad(prefix || "body");
  const allowed = patch ? ["title", "description", "priority", "labels", "assigneeId"] :
    ["title", "description", "priority", "labels", "assigneeId"];
  for (const k of Object.keys(b)) if (!allowed.includes(k)) bad(prefix ? `${prefix}.${k}` : k);
  const field = (x: string) => prefix ? `${prefix}.${x}` : x;
  if (!patch || "title" in b) {
    if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200) bad(field("title"));
  }
  if ("description" in b && typeof b.description !== "string") bad(field("description"));
  if ("priority" in b && !["low", "normal", "high"].includes(b.priority)) bad(field("priority"));
  if ("labels" in b) {
    if (!Array.isArray(b.labels) || b.labels.some((x: any) => typeof x !== "string" ||
      !x.trim() || x.trim().length > 30 || /\s/.test(x.trim()))) bad(field("labels"));
  }
  if ("assigneeId" in b && b.assigneeId !== null) {
    const project = s.projects.find((p) => p.id === projectId);
    const member = project && (project.ownerId === b.assigneeId || s.members.get(projectId)?.has(b.assigneeId));
    if (!Number.isInteger(b.assigneeId) || !member) bad(field("assigneeId"));
  }
}
export function createTask(s: State, p: Project, u: number, b: any): Task {
  const n = p.next++;
  const t: Task = {
    id: ++s.ids.task, key: `${p.key}-${n}`, number: n, projectId: p.id,
    title: b.title.trim(), description: b.description ?? "", status: "open",
    priority: b.priority ?? "normal",
    labels: [...new Set<string>((b.labels ?? [] as string[])
      .map((x: string) => x.trim().toLowerCase()))].sort(),
    assigneeId: b.assigneeId ?? null, createdBy: u, createdAt: iso(s), updatedAt: iso(s),
    version: 1, deleted: false,
  };
  s.tasks.push(t); s.histories.set(t.id, []); s.comments.set(t.id, []); s.deps.set(t.id, new Set());
  return t;
}
