import type { State } from "./model";
import { response, validation } from "./http";

const priorities = ["high", "normal", "low"];
const statuses = ["open", "in_progress", "done"];
export function list(s: State, pid: number, uid: number, url: URL): Response {
  const q = url.searchParams;
  let tasks = s.tasks.filter((t) => t.projectId === pid);
  const status = q.get("status");
  if (status && !statuses.includes(status)) validation("status");
  if (status) tasks = tasks.filter((t) => t.status === status);
  const assignee = q.get("assignee");
  if (assignee) {
    let target: number | null;
    if (assignee === "me") target = uid;
    else if (assignee === "none") target = null;
    else if (/^\d+$/.test(assignee)) target = Number(assignee);
    else return validation("assignee");
    tasks = tasks.filter((t) => t.assigneeId === target);
  }
  const label = q.get("label")?.toLowerCase();
  if (label) tasks = tasks.filter((t) => t.labels.includes(label));
  const search = q.get("q")?.toLowerCase();
  if (search) tasks = tasks.filter((t) => `${t.title} ${t.description}`.toLowerCase().includes(search));
  const sort = q.get("sort") || "created";
  if (! ["created", "-created", "priority"].includes(sort)) validation("sort");
  tasks.sort((a, b) => sort === "-created" ? b.number - a.number : sort === "priority" ?
    priorities.indexOf(a.priority) - priorities.indexOf(b.priority) || a.number - b.number : a.number - b.number);
  const limitText = q.get("limit") || "20", limit = Number(limitText);
  if (!/^\d+$/.test(limitText) || limit < 1 || limit > 100) validation("limit");
  const cursor = q.get("cursor");
  let offset = 0;
  if (cursor) {
    try {
      const c = JSON.parse(atob(cursor));
      if (c.sort !== sort || c.key !== filterKey(url)) validation("cursor");
      offset = c.offset;
    }
    catch { validation("cursor"); }
  }
  const items = tasks.slice(offset, offset + limit);
  const nextCursor = offset + limit < tasks.length ?
    btoa(JSON.stringify({ offset: offset + limit, sort, key: filterKey(url) })) : null;
  return response({ items, nextCursor });
}
function filterKey(url: URL) {
  return [...url.searchParams.entries()].filter(([k]) => k !== "cursor" && k !== "limit")
    .sort(([a], [b]) => a.localeCompare(b)).map((x) => x.join("=")).join("&");
}
