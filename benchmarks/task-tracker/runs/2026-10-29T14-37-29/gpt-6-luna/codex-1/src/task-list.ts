import { invalid, response } from "./errors";
import type { Context } from "./model";

const priorities = ["high", "normal", "low"];

export function list(ctx: Context, url: URL, projectId: number, userId: number) {
  const q = url.searchParams;
  const status = q.get("status");
  const sort = q.get("sort") ?? "created";
  let limit = 20;
  if (q.has("limit")) {
    const raw = q.get("limit")!;
    limit = Number(raw);
    if (!/^\d+$/.test(raw) || limit < 1 || limit > 100) invalid("limit");
  }
  if (status && !["open", "in_progress", "done"].includes(status)) invalid("status");
  if (!(["created", "-created", "priority"] as string[]).includes(sort)) invalid("sort");
  const assignee = q.get("assignee");
  if (assignee && assignee !== "me" && assignee !== "none" && !/^\d+$/.test(assignee)) invalid("assignee");
  let items = ctx.state.tasks.filter((t) => t.projectId === projectId);
  if (status) items = items.filter((t) => t.status === status);
  if (assignee === "me") items = items.filter((t) => t.assigneeId === userId);
  if (assignee === "none") items = items.filter((t) => t.assigneeId === null);
  if (assignee && /^\d+$/.test(assignee)) items = items.filter((t) => t.assigneeId === Number(assignee));
  const label = q.get("label")?.toLowerCase();
  if (label) items = items.filter((t) => t.labels.includes(label));
  const search = q.get("q")?.toLowerCase();
  if (search) items = items.filter((t) => `${t.title} ${t.description ?? ""}`.toLowerCase().includes(search));
  if (sort === "-created") items.reverse();
  if (sort === "priority") {
    items.sort((a, b) => priorities.indexOf(a.priority) - priorities.indexOf(b.priority) ||
      a.number - b.number);
  }
  const cursor = q.get("cursor");
  let offset = 0;
  if (cursor) {
    try { offset = Number(atob(cursor)); } catch { invalid("cursor"); }
    if (!Number.isInteger(offset) || offset < 0) invalid("cursor");
  }
  const page = items.slice(offset, offset + limit);
  const next = offset + limit < items.length ? btoa(String(offset + limit)) : null;
  return response({ items: page, nextCursor: next });
}
export function stats(ctx: Context, projectId: number) {
  const items = ctx.state.tasks.filter((t) => t.projectId === projectId);
  const count = (field: "status" | "priority", values: string[]) => Object.fromEntries(
    values.map((value) => [value, items.filter((t) => t[field] === value).length]));
  const assignees = new Map<number, number>();
  for (const t of items) if (t.assigneeId !== null) {
    assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0) + 1);
  }
  return response({ total: items.length,
    byStatus: count("status", ["open", "in_progress", "done"]),
    byPriority: count("priority", ["high", "normal", "low"]),
    byAssignee: [...assignees].map(([userId, count]) => ({ userId, count }))
      .sort((a, b) => b.count - a.count || a.userId - b.userId) });
}
