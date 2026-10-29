import type { State, Task } from "./model";
import { iso, taskByKey } from "./model";
import { bad, body, empty, fail, json } from "./http";

export function list(req: Request, s: State, projectId: number, user: number) {
  const url = new URL(req.url), q = url.searchParams;
  const status = q.get("status"), assignee = q.get("assignee"), label = q.get("label"), search = q.get("q");
  const sort = q.get("sort") ?? "created", limitText = q.get("limit") ?? "20";
  if (status && !["open", "in_progress", "done"].includes(status)) bad("status");
  if (!(["created", "-created", "priority"].includes(sort))) bad("sort");
  if (!/^\d+$/.test(limitText) || Number(limitText) < 1 || Number(limitText) > 100) bad("limit");
  if (assignee && assignee !== "me" && assignee !== "none" && !/^\d+$/.test(assignee)) bad("assignee");
  let items = s.tasks.filter((t) => !t.deleted && t.projectId === projectId);
  if (status) items = items.filter((t) => t.status === status);
  if (assignee === "none") items = items.filter((t) => t.assigneeId === null);
  else if (assignee === "me") items = items.filter((t) => t.assigneeId === user);
  else if (assignee) items = items.filter((t) => t.assigneeId === Number(assignee));
  if (label) items = items.filter((t) => t.labels.includes(label.toLowerCase()));
  if (search) items = items.filter((t) => `${t.title} ${t.description}`.toLowerCase().includes(search.toLowerCase()));
  if (sort === "-created") items.reverse();
  if (sort === "priority") {
    const rank: Record<string, number> = { high: 0, normal: 1, low: 2 };
    items.sort((a, b) => rank[a.priority]! - rank[b.priority]! || a.number - b.number);
  }
  const cursor = q.get("cursor");
  if (cursor) {
    try {
      const decoded = JSON.parse(atob(cursor));
      if (decoded.sort !== sort || decoded.filter !== `${status}|${assignee}|${label}|${search}`) bad("cursor");
      const index = items.findIndex((t) => t.id === decoded.id);
      if (index < 0) bad("cursor");
      items = items.slice(index + 1);
    } catch { bad("cursor"); }
  }
  const size = Number(limitText), page = items.slice(0, size), more = items.length > size;
  const nextCursor = more ? btoa(JSON.stringify({ id: page.at(-1)!.id, sort,
    filter: `${status}|${assignee}|${label}|${search}` })) : null;
  return json({ items: page.map(({ deleted, ...t }) => t), nextCursor });
}
export async function comments(req: Request, s: State, t: Task, u: number, action?: string) {
  const arr = s.comments.get(t.id)!;
  if (req.method === "GET") return json(arr);
  if (req.method !== "POST") return undefined;
  const b = await body(req);
  if (typeof b.body !== "string" || !b.body.trim() || b.body.length > 2000) bad("body");
  const item = { id: ++s.ids.comment, authorId: u, body: b.body, createdAt: iso(s) };
  arr.push(item); return json(item, 201);
}
export async function dependencies(req: Request, s: State, t: Task, _u: number, other?: string) {
  const ids = s.deps.get(t.id)!;
  if (req.method === "GET") {
    const from = (id: number) => s.tasks.find((x) => x.id === id && !x.deleted);
    const name = (x: Task) => x.number;
    const dependsOn = [...ids].map(from).filter((x): x is Task => !!x).sort((a,b)=>name(a)-name(b)).map(x=>x.key);
    const blocks = s.tasks.filter((x) => !x.deleted && s.deps.get(x.id)?.has(t.id))
      .sort((a,b)=>a.number-b.number).map(x=>x.key);
    return json({ key: t.key, dependsOn, blocks });
  }
  if (req.method === "POST" && !other) {
    const b = await body(req);
    if (typeof b.dependsOn !== "string") bad("dependsOn");
    const dep = taskByKey(s, b.dependsOn);
    if (!dep || dep.projectId !== t.projectId || dep.id === t.id) bad("dependsOn");
    if (ids.has(dep!.id)) fail(409, "conflict", "Dependency already exists");
    if (reaches(s, dep!.id, t.id)) fail(409, "dependency_cycle", "Dependency would create a cycle");
    ids.add(dep!.id);
    const dependsOn = [...ids].map((id) => s.tasks.find((x) => x.id === id)!)
      .sort((a, b) => a.number - b.number).map((x) => x.key);
    return json({ key: t.key, dependsOn, blocks: [] }, 201);
  }
  if (req.method === "DELETE" && other) {
    const dep = taskByKey(s, other);
    if (!dep || !ids.delete(dep.id)) fail(404, "not_found", "Dependency not found");
    return empty();
  }
  return undefined;
}
function reaches(s: State, start: number, goal: number, seen = new Set<number>()): boolean {
  if (start === goal) return true;
  if (seen.has(start)) return false;
  seen.add(start);
  for (const next of s.deps.get(start) ?? []) if (reaches(s, next, goal, seen)) return true;
  return false;
}
