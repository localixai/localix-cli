import { iso, labels, validation, type Ctx, type Task } from "./model";
const priorities = ["low", "normal", "high"];
export function make(c: Ctx, p: any, b: any): Task | Response {
  if (
    typeof b.title !== "string" ||
    !b.title.trim() ||
    b.title.trim().length > 200
  )
    return validation("title");
  if (b.description !== undefined && typeof b.description !== "string")
    return validation("description");
  if (b.priority !== undefined && !priorities.includes(b.priority))
    return validation("priority");
  const ls = b.labels === undefined ? [] : labels(b.labels);
  if (!ls) return validation("labels");
  if (
    b.assigneeId !== undefined &&
    b.assigneeId !== null &&
    !c.db.members.get(p.id)?.has(b.assigneeId)
  )
    return validation("assigneeId");
  const n = p.next++;
  const t: Task = {
    id: c.db.tasks.length + 1,
    key: `${p.key}-${n}`,
    number: n,
    projectId: p.id,
    title: b.title.trim(),
    description: b.description ?? "",
    status: "open",
    priority: b.priority ?? "normal",
    labels: ls,
    assigneeId: b.assigneeId ?? null,
    createdBy: c.user!.id,
    createdAt: iso(c),
    updatedAt: iso(c),
    version: 1,
    depends: [],
    comments: [],
    history: [],
  };
  c.db.tasks.push(t);
  return t;
}
export const output = (t: Task) => {
  const { deleted, depends, comments, history, ...out } = t;
  return out;
};
