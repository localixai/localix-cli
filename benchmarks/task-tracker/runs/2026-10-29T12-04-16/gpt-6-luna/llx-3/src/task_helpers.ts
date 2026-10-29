import type { State, Task } from "./model";
import { stamp } from "./common";

export function validate(b: any, prefix = ""): string | null {
  if (!b || typeof b !== "object" || Array.isArray(b)) return "body";
  const title = typeof b.title === "string" ? b.title.trim() : "";
  if ("title" in b && (!title || title.length > 200)) return prefix + "title";
  if ("description" in b && typeof b.description !== "string") return prefix + "description";
  if ("priority" in b && !["low", "normal", "high"].includes(b.priority)) return prefix + "priority";
  if ("labels" in b && (!Array.isArray(b.labels) ||
    b.labels.some((x: any) => typeof x !== "string" || !x.trim() ||
      x.trim().length > 30 || /\s/.test(x.trim()))))
    return prefix + "labels";
  return null;
}

export function create(s: State, p: number, u: number, b: any): Task {
  const project = s.projects.find(x => x.id === p)!;
  const n = (s.sequences.get(p) ?? 0) + 1;
  s.sequences.set(p, n);
  const at = stamp(s);
  const labels: string[] = [...new Set<string>((b.labels ?? []).map((x: string) =>
    x.trim().toLowerCase()))].sort();
  const t: Task = { id: s.tasks.reduce((v, x) => Math.max(v, x.id), 0) + 1,
    key: `${project.key}-${n}`, number: n, projectId: p, title: b.title.trim(),
    description: b.description ?? "", status: "open", priority: b.priority ?? "normal",
    labels, assigneeId: b.assigneeId ?? null, createdBy: u, createdAt: at, updatedAt: at,
    version: 1, deps: [], history: [] };
  s.tasks.push(t);
  return t;
}
