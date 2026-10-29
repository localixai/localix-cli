import { State, Task, Project, taskView } from "./state";
import { answer, auth, fail, project } from "./http";
export function stats(s: State, r: Request, id: string) {
  const u = auth(s, r),
    p = project(s, id, u.id),
    all = s.tasks.filter((t) => t.projectId === p.id);
  const byStatus: any = { open: 0, in_progress: 0, done: 0 },
    byPriority: any = { high: 0, normal: 0, low: 0 },
    ass = new Map<number, number>();
  for (const t of all) {
    byStatus[t.status]++;
    byPriority[t.priority]++;
    if (t.assigneeId !== null)
      ass.set(t.assigneeId, (ass.get(t.assigneeId) || 0) + 1);
  }
  return answer({
    total: all.length,
    byStatus,
    byPriority,
    byAssignee: [...ass]
      .sort((a, b) => b[1] - a[1] || a[0] - b[0])
      .map(([userId, count]) => ({ userId, count })),
  });
}
export function listing(s: State, url: URL, p: Project, user: number) {
  const q = url.searchParams;
  let all = s.tasks.filter((t) => t.projectId === p.id);
  const status = q.get("status"),
    ass = q.get("assignee"),
    label = q.get("label")?.toLowerCase(),
    text = q.get("q")?.toLowerCase();
  if (status && !["open", "in_progress", "done"].includes(status))
    fail(400, "validation", "Invalid status", "status");
  if (ass && ass !== "none" && ass !== "me" && !/^\d+$/.test(ass))
    fail(400, "validation", "Invalid assignee", "assignee");
  if (status) all = all.filter((t) => t.status === status);
  if (ass === "none") all = all.filter((t) => t.assigneeId === null);
  if (ass === "me") all = all.filter((t) => t.assigneeId === user);
  else if (ass && ass !== "none")
    all = all.filter((t) => t.assigneeId === Number(ass));
  if (label) all = all.filter((t) => t.labels.includes(label));
  if (text)
    all = all.filter((t) =>
      `${t.title} ${t.description}`.toLowerCase().includes(text),
    );
  const sort = q.get("sort") || "created";
  if (!["created", "-created", "priority"].includes(sort))
    fail(400, "validation", "Invalid sort", "sort");
  all.sort((a, b) =>
    sort === "-created"
      ? b.number - a.number
      : sort === "priority"
        ? ["high", "normal", "low"].indexOf(a.priority) -
            ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number
        : a.number - b.number,
  );
  let limit = 20;
  if (q.has("limit")) {
    const raw = q.get("limit")!;
    limit = Number(raw);
    if (!/^\d+$/.test(raw) || limit < 1 || limit > 100)
      fail(400, "validation", "Invalid limit", "limit");
  }
  const offset = q.get("cursor") ? Number(atob(q.get("cursor")!)) : 0;
  if (
    !Number.isInteger(offset) ||
    offset < 0 ||
    (q.get("cursor") && !/^\d+$/.test(atob(q.get("cursor")!)))
  )
    fail(400, "validation", "Invalid cursor", "cursor");
  const page = all.slice(offset, offset + limit),
    next = offset + limit < all.length ? btoa(String(offset + limit)) : null;
  return answer({ items: page.map(taskView), nextCursor: next });
}
