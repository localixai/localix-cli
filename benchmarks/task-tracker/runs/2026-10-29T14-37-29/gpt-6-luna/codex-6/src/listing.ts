import type { Ctx, Result, Task } from "./types";
import { fail } from "./util";
export function listRoute(c: Ctx, p: number): Result {
  const q = c.url.searchParams,
    status = q.get("status"),
    assignee = q.get("assignee"),
    label = q.get("label"),
    term = q.get("q")?.toLowerCase(),
    sort = q.get("sort") || "created",
    lim = q.get("limit") || "20";
  if (status && !["open", "in_progress", "done"].includes(status))
    return fail(400, "validation", "invalid status", "status");
  if (!["created", "-created", "priority"].includes(sort))
    return fail(400, "validation", "invalid sort", "sort");
  if (!/^\d+$/.test(lim) || +lim < 1 || +lim > 100)
    return fail(400, "validation", "invalid limit", "limit");
  if (
    assignee &&
    assignee !== "me" &&
    assignee !== "none" &&
    !/^\d+$/.test(assignee)
  )
    return fail(400, "validation", "invalid assignee", "assignee");
  let all = c.state.tasks.filter((t) => t.projectId === p);
  if (status) all = all.filter((t) => t.status === status);
  if (assignee === "me") all = all.filter((t) => t.assigneeId === c.user!.id);
  else if (assignee === "none") all = all.filter((t) => t.assigneeId === null);
  else if (assignee) all = all.filter((t) => t.assigneeId === +assignee);
  if (label) all = all.filter((t) => t.labels.includes(label.toLowerCase()));
  if (term)
    all = all.filter((t) =>
      (t.title + " " + t.description).toLowerCase().includes(term),
    );
  if (sort === "-created") all.sort((a, b) => b.number - a.number);
  else if (sort === "priority") {
    const rank: Record<string, number> = { high: 0, normal: 1, low: 2 };
    all.sort(
      (a, b) => rank[a.priority]! - rank[b.priority]! || a.number - b.number,
    );
  } else all.sort((a, b) => a.number - b.number);
  const cursor = q.get("cursor");
  let offset = 0;
  if (cursor) {
    try {
      const data = JSON.parse(atob(cursor));
      if (
        data.sort !== sort ||
        data.q !== (term || "") ||
        data.assignee !== (assignee || "") ||
        data.label !== (label || "") ||
        data.status !== (status || "")
      )
        throw 0;
      offset = data.offset;
    } catch {
      return fail(400, "validation", "invalid cursor", "cursor");
    }
  }
  const items = all.slice(offset, offset + +lim),
    nextCursor =
      offset + items.length < all.length
        ? btoa(
            JSON.stringify({
              sort,
              q: term || "",
              assignee: assignee || "",
              label: label || "",
              status: status || "",
              offset: offset + items.length,
            }),
          )
        : null;
  return { status: 200, body: { items, nextCursor } };
}
