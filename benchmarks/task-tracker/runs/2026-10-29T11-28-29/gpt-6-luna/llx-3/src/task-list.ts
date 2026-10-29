import { State, bad, error, json, memberRole } from "./model";
export function listTasks(s: State, p: any, url: URL, uid: number) {
  let tasks = s.tasks.filter((t) => t.projectId === p.id);
  const q = url.searchParams;
  const status = q.get("status"), assignee = q.get("assignee"), label = q.get("label"), search = q.get("q");
  const sort = q.get("sort") || "created", limitText = q.get("limit") || "20", limit = Number(limitText);
  if (status && !["open", "in_progress", "done"].includes(status)) return bad("status");
  if (!["created", "-created", "priority"].includes(sort)) return bad("sort");
  if (!/^\d+$/.test(limitText) || limit < 1 || limit > 100) return bad("limit");
  if (assignee && assignee !== "me" && assignee !== "none" && !/^\d+$/.test(assignee)) return bad("assignee");
  if (status) tasks = tasks.filter((t) => t.status === status);
  if (assignee) tasks = tasks.filter((t) => assignee === "none" ? t.assigneeId === null :
    t.assigneeId === (assignee === "me" ? uid : Number(assignee)));
  if (label) tasks = tasks.filter((t) => t.labels.includes(label.toLowerCase()));
  if (search) tasks = tasks.filter((t) => `${t.title} ${t.description}`.toLowerCase().includes(search.toLowerCase()));
  if (sort === "-created") tasks.reverse();
  if (sort === "priority") tasks.sort((a, b) =>
    ["high", "normal", "low"].indexOf(a.priority) -
    ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number);
  let offset = 0;
  const cursor = q.get("cursor");
  if (cursor) {
    try {
      const c = JSON.parse(atob(cursor));
      if (c.sort !== sort || c.limit !== limit ||
          c.filter !== `${status}|${assignee}|${label}|${search}`) throw 0;
      offset = c.offset;
    }
    catch { return bad("cursor"); }
  }
  const items = tasks.slice(offset, offset + limit), more = offset + limit < tasks.length;
  const nextCursor = more ? btoa(JSON.stringify({ offset: offset + limit, sort, limit,
    filter: `${status}|${assignee}|${label}|${search}` })) : null;
  return json({ items, nextCursor });
}
