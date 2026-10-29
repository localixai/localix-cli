import type { State } from "./state";
import { bad, err, iso } from "./util";

export function validateTask(b: any, s: State, p: any) {
  if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200) return "title";
  if (b.description !== undefined && typeof b.description !== "string") return "description";
  if (b.priority !== undefined && !["low", "normal", "high"].includes(b.priority)) return "priority";
  if (b.labels !== undefined && (!Array.isArray(b.labels) || b.labels.some((x: any) =>
    typeof x !== "string" || !x.trim() || x.trim().length > 30 || /\s/.test(x.trim())))) return "labels";
  if (b.assigneeId != null && !(p.ownerId === b.assigneeId ||
    p.members.some((x: any) => x.userId === b.assigneeId))) return "assigneeId";
  return "";
}
export function makeTask(s: State, p: any, u: any, b: any) {
  const number = (s.counts.get(p.id) ?? 0) + 1;
  s.counts.set(p.id, number);
  const labels = [...new Set((b.labels ?? []).map((x: string) => x.trim().toLowerCase()))].sort();
  const t = { id: s.tasks.length + 1, key: `${p.key}-${number}`, number, projectId: p.id,
    title: b.title.trim(), description: b.description ?? "", status: "open",
    priority: b.priority ?? "normal", labels, assigneeId: b.assigneeId ?? null,
    createdBy: u.id, createdAt: iso(s), updatedAt: iso(s), version: 1, deleted: false };
  Object.defineProperty(t, "deleted", { value: false, writable: true, enumerable: false });
  s.tasks.push(t); return t;
}
export function permission(p: any, u: any) {
  if (p.ownerId === u.id) return "owner";
  return p.members.find((m: any) => m.userId === u.id)?.role;
}
