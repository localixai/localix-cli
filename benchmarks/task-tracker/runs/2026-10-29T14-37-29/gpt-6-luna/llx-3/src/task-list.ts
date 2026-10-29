import type { State } from "./state";
import { bad, json } from "./util";

export function taskList(_req: Request, s: State, p: any, u: any, url: URL) {
  const q = url.searchParams, limitText = q.get("limit") ?? "20", limit = Number(limitText);
  const status = q.get("status"), sort = q.get("sort") ?? "created";
  const assignee = q.get("assignee"), label = q.get("label")?.toLowerCase(), search = q.get("q")?.toLowerCase();
  if (status && !["open", "in_progress", "done"].includes(status)) return bad("status");
  if (!["created", "-created", "priority"].includes(sort)) return bad("sort");
  if (!/^\d+$/.test(limitText) || limit < 1 || limit > 100) return bad("limit");
  if (assignee && assignee !== "me" && assignee !== "none" && !/^\d+$/.test(assignee)) return bad("assignee");
  const all = s.tasks.filter(t => t.projectId === p.id && !t.deleted &&
    (!status || t.status === status) && (!label || t.labels.includes(label)) &&
    (!search || `${t.title} ${t.description}`.toLowerCase().includes(search)) &&
    (!assignee || (assignee === "none" ? t.assigneeId === null :
      t.assigneeId === Number(assignee === "me" ? u.id : assignee))));
  const rank: any = { high: 0, normal: 1, low: 2 };
  all.sort((a, b) => sort === "priority" ? rank[a.priority] - rank[b.priority] || a.number - b.number :
    sort === "-created" ? b.number - a.number : a.number - b.number);
  const cursor = q.get("cursor");
  let start = 0;
  if (cursor) {
    try { start = Number(JSON.parse(atob(cursor)).i); } catch { return bad("cursor"); }
    if (!Number.isInteger(start) || start < 0) return bad("cursor");
  }
  const items = all.slice(start, start + limit), next = start + limit < all.length ?
    btoa(JSON.stringify({ i: start + limit, sort, status, assignee, label, search })) : null;
  return json({ items, nextCursor: next });
}
