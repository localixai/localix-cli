import { member, bad, ok, type Ctx } from "./http";
import { taskView } from "./http";

export function listing(c: Ctx, pid: number) {
  const p = member(c, pid)!;
  const q = c.url.searchParams;
  let rows = c.s.tasks.filter((t) => t.projectId === pid);
  const status = q.get("status");
  if (status) {
    if (!["open", "in_progress", "done"].includes(status)) return bad("status");
    rows = rows.filter((t) => t.status === status);
  }
  const as = q.get("assignee");
  if (as) {
    if (as === "me") rows = rows.filter((t) => t.assigneeId === c.user);
    else if (as === "none") rows = rows.filter((t) => t.assigneeId === null);
    else if (/^\d+$/.test(as))
      rows = rows.filter((t) => t.assigneeId === Number(as));
    else return bad("assignee");
  }
  const label = q.get("label");
  if (label) rows = rows.filter((t) => t.labels.includes(label.toLowerCase()));
  const search = q.get("q")?.toLowerCase();
  if (search)
    rows = rows.filter((t) =>
      `${t.title} ${t.description}`.toLowerCase().includes(search),
    );
  const sort = q.get("sort") || "number";
  if (!["number", "-created", "priority"].includes(sort)) return bad("sort");
  if (sort === "-created") rows.sort((a, b) => b.number - a.number);
  else if (sort === "priority")
    rows.sort(
      (a, b) =>
        ["high", "normal", "low"].indexOf(a.priority) -
          ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number,
    );
  else rows.sort((a, b) => a.number - b.number);
  let offset = 0;
  const cursor = q.get("cursor");
  if (cursor) {
    try {
      const cdata = JSON.parse(atob(cursor));
      if (
        cdata.sort !== sort ||
        cdata.filters !== q.toString().replace(/&cursor=[^&]*/, "")
      )
        return bad("cursor");
      offset = cdata.offset;
    } catch {
      return bad("cursor");
    }
  }
  const limitRaw = q.get("limit") || "20",
    limit = Number(limitRaw);
  if (!/^\d+$/.test(limitRaw) || limit < 1 || limit > 100) return bad("limit");
  const page = rows.slice(offset, offset + limit),
    has = offset + limit < rows.length;
  const filters = new URLSearchParams(q);
  filters.delete("cursor");
  const next = has
    ? btoa(
        JSON.stringify({
          sort,
          filters: filters.toString(),
          offset: offset + limit,
        }),
      )
    : null;
  return ok({ items: page.map(taskView), nextCursor: next });
}
export function stats(c: Ctx, pid: number) {
  const rows = c.s.tasks.filter((t) => t.projectId === pid),
    byStatus = { open: 0, in_progress: 0, done: 0 },
    byPriority = { low: 0, normal: 0, high: 0 };
  for (const t of rows) {
    (byStatus as any)[t.status]++;
    (byPriority as any)[t.priority]++;
  }
  const counts = new Map<number, number>();
  for (const t of rows)
    if (t.assigneeId !== null)
      counts.set(t.assigneeId, (counts.get(t.assigneeId) || 0) + 1);
  return ok({
    total: rows.length,
    byStatus,
    byPriority,
    byAssignee: [...counts]
      .sort((a, b) => b[1] - a[1] || a[0] - b[0])
      .map(([userId, count]) => ({ userId, count })),
  });
}
