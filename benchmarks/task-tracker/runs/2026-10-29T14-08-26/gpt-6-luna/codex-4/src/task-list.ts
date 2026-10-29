import type { State } from "./model";
import { taskView } from "./model";
import { ApiError, respond } from "./http";
export function listTasks(
  url: URL,
  s: State,
  id: number,
  uid: number,
): Response {
  const q = url.searchParams;
  let items = s.tasks.filter((t) => t.projectId === id);
  if (q.has("status")) {
    const v = q.get("status")!;
    if (!["open", "in_progress", "done"].includes(v))
      throw new ApiError(400, "validation", "status is invalid", "status");
    items = items.filter((t) => t.status === v);
  }
  if (q.has("priority")) {
    const v = q.get("priority")!;
    if (!["high", "normal", "low"].includes(v))
      throw new ApiError(400, "validation", "priority is invalid", "priority");
    items = items.filter((t) => t.priority === v);
  }
  if (q.has("assignee")) {
    const v = q.get("assignee")!;
    if (!["none", "me"].includes(v) && !/^\d+$/.test(v))
      throw new ApiError(400, "validation", "assignee is invalid", "assignee");
    items = items.filter((t) =>
      v === "none"
        ? t.assigneeId === null
        : v === "me"
          ? t.assigneeId === uid
          : t.assigneeId === Number(v),
    );
  }
  if (q.has("label"))
    items = items.filter((t) =>
      t.labels.includes(q.get("label")!.toLowerCase()),
    );
  if (q.has("q")) {
    const term = q.get("q")!.toLowerCase();
    items = items.filter((t) =>
      `${t.title} ${t.description}`.toLowerCase().includes(term),
    );
  }
  const sort = q.get("sort") || "created";
  if (!["created", "-created", "priority"].includes(sort))
    throw new ApiError(400, "validation", "sort is invalid", "sort");
  const weights: Record<string, number> = { high: 0, normal: 1, low: 2 };
  items.sort((a, b) =>
    sort === "-created"
      ? b.number - a.number
      : sort === "priority"
        ? weights[a.priority]! - weights[b.priority]! || a.number - b.number
        : a.number - b.number,
  );
  const limit = Number(q.get("limit") || 20);
  if (!Number.isInteger(limit) || limit < 1 || limit > 100)
    throw new ApiError(400, "validation", "limit is invalid", "limit");
  let start = 0;
  const cursor = q.get("cursor");
  if (cursor) {
    try {
      const decoded = atob(cursor);
      if (!/^\d+$/.test(decoded)) throw new Error();
      start = Number(decoded);
    } catch {
      throw new ApiError(400, "validation", "cursor is invalid", "cursor");
    }
  }
  const page = items.slice(start, start + limit),
    next = start + limit < items.length ? btoa(String(start + limit)) : null;
  return respond({ items: page.map(taskView), nextCursor: next });
}
