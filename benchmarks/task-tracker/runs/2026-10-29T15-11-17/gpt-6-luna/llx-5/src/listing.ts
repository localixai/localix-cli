import type { Store, Project, Task } from "./model";
import { required } from "./http";

const output = (t: Task) => {
  const { deleted, deps, ...body } = t;
  return body;
};
export function listTasks(url: URL, s: Store, p: Project, user: number) {
  let items = s.tasks.filter((t) => t.projectId === p.id && !t.deleted);
  const q = url.searchParams;
  if (q.has("status")) {
    const v = q.get("status")!;
    required(v, "status", (x) => ["open", "in_progress", "done"].includes(x));
    items = items.filter((t) => t.status === v);
  }
  if (q.has("label")) items = items.filter((t) => t.labels.includes(q.get("label")!.toLowerCase()));
  if (q.has("q")) {
    const v = q.get("q")!.toLowerCase();
    items = items.filter((t) => `${t.title} ${t.description}`.toLowerCase().includes(v));
  }
  if (q.has("assignee")) {
    const v = q.get("assignee")!;
    if (v === "me") items = items.filter((t) => t.assigneeId === user);
    else if (v === "none") items = items.filter((t) => t.assigneeId === null);
    else {
      required(v, "assignee", (x) => /^\d+$/.test(x));
      items = items.filter((t) => t.assigneeId === +v);
    }
  }
  const sort = q.get("sort") ?? "created";
  required(sort, "sort", (v) => ["created", "-created", "priority"].includes(v));
  if (sort === "-created") items.reverse();
  if (sort === "priority") items.sort((a, b) =>
    ["high", "normal", "low"].indexOf(a.priority) -
      ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number);
  const limit = q.has("limit") ? Number(q.get("limit")) : 20;
  required(limit, "limit", (v) => Number.isInteger(v) && v >= 1 && v <= 100);
  const cursor = q.get("cursor");
  if (cursor) {
    const n = Number(atob(cursor));
    required(Number.isInteger(n), "cursor", (v) => v === true);
    items = items.slice(n);
  }
  const more = items.length > limit, page = items.slice(0, limit);
  const offset = cursor ? Number(atob(cursor)) : 0;
  return { items: page.map(output), nextCursor: more ? btoa(String(offset + limit)) : null };
}
