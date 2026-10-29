import type { Store, User, Project, Task } from "./model";
import { stamp, visible } from "./model";
import { body, bad, error, json, task, role, precondition } from "./http";

export async function taskRoutes(
  req: Request, s: Store, path: string[], u: User, p: Project,
): Promise<Response> {
  if (path[2] === "stats") return stats(s, p);
  if (path.length === 3 && req.method === "POST") return create(req, s, p, u);
  if (path.length === 4 && path[3] === "bulk" && req.method === "POST") {
    if (role(p, u) === "viewer") return error("forbidden", "forbidden", 403);
    const b = await body(req);
    if (!b || !Array.isArray(b.tasks) || b.tasks.length < 1 || b.tasks.length > 50) return bad("tasks");
    const checked = b.tasks.map((v: any, i: number) => validate(v, p, s, `tasks[${i}]`));
    const invalid = checked.find((x: any) => x instanceof Response);
    if (invalid) return invalid;
    return json({ items: checked.map((v: any) => makeTask(s, p, u, v)) }, 201);
  }
  if (path.length === 3 && req.method === "GET") return listing(req, s, p, u, new URL(req.url));
  const t = task(s, path[3] || "");
  if (!t || t.projectId !== p.id) return error("not_found", "not found", 404);
  if (path.length === 4 && req.method === "GET")
    return json(visible(t), 200, { ETag: `"${t.version}"` });
  if (path.length === 4 && req.method === "PATCH") return patch(req, s, t, u);
  if (path.length === 4 && req.method === "DELETE") {
    if (role(p, u) === "viewer" || (role(p, u) !== "owner" && t.createdBy !== u.id))
      return error("forbidden", "forbidden", 403);
    t.deleted = true;
    for (const a of s.tasks) {
      (a as any).dependsOn = ((a as any).dependsOn || []).filter((id: number) => id !== t.id);
    }
    return new Response(null, { status: 204 });
  }
  if (path[4] === "transitions") return transition(req, s, t, u, p);
  if (path[4] === "history" && req.method === "GET") return json(s.history.get(t.id) || []);
  if (path[4] === "comments") return comments(req, s, t, u);
  if (path[4] === "dependencies") return dependencies(req, s, t, u, p, path[5]);
  return error("not_found", "not found", 404);
}
export async function tasks(req: Request, s: Store, path: string[], u: User, url: URL) {
  if (path[1] === "projects") return error("not_found", "not found", 404);
  const t = task(s, path[1] || "");
  if (!t) return error("not_found", "not found", 404);
  const p = s.projects.find(x => x.id === t.projectId)!;
  if (!role(p, u)) return error("not_found", "not found", 404);
  return taskRoutes(req, s, ["projects", String(p.id), "tasks", ...path.slice(1)], u, p);
}
function dependenciesFor(s: Store, t: Task): Task[] {
  const ids = (t as any).dependsOn as number[] || [];
  return ids.map(id => s.tasks.find(x => x.id === id)).filter(Boolean) as Task[];
}
function stats(s: Store, p: Project) {
  const ts = s.tasks.filter(t => t.projectId === p.id && !t.deleted);
  const count = (k: string, values: string[]) => Object.fromEntries(values.map(v =>
    [v, ts.filter(t => (t as any)[k] === v).length]));
  const assignees = new Map<number, number>();
  for (const t of ts) if (t.assigneeId) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) || 0) + 1);
  return json({ total: ts.length, byStatus: count("status", ["open", "in_progress", "done"]),
    byPriority: count("priority", ["high", "normal", "low"]),
    byAssignee: [...assignees].map(([userId, count]) => ({ userId, count }))
      .sort((a, b) => b.count - a.count || a.userId - b.userId) });
}
function listing(req: Request, s: Store, p: Project, u: User, url: URL) {
  const q = url.searchParams, status = q.get("status"), sort = q.get("sort");
  const rawLimit = q.get("limit") || "20", limit = Number(rawLimit);
  const assignee = q.get("assignee");
  if (status && !["open", "in_progress", "done"].includes(status)) return bad("status");
  if (sort && !["-created", "priority"].includes(sort)) return bad("sort");
  if (!/^\d+$/.test(rawLimit) || limit < 1 || limit > 100) return bad("limit");
  if (assignee && assignee !== "me" && assignee !== "none" && !/^\d+$/.test(assignee)) return bad("assignee");
  let all = s.tasks.filter(t => t.projectId === p.id && !t.deleted);
  if (status) all = all.filter(t => t.status === status);
  if (assignee) all = all.filter(t => assignee === "none" ? !t.assigneeId :
    t.assigneeId === (assignee === "me" ? u.id : Number(assignee)));
  const label = q.get("label")?.toLowerCase(), text = q.get("q")?.toLowerCase();
  if (label) all = all.filter(t => t.labels.includes(label));
  if (text) all = all.filter(t => `${t.title} ${t.description}`.toLowerCase().includes(text));
  if (sort === "-created") all.reverse();
  if (sort === "priority") all.sort((a, b) =>
    ["high", "normal", "low"].indexOf(a.priority) - ["high", "normal", "low"].indexOf(b.priority) ||
    a.number - b.number);
  const filters = Object.fromEntries([...q].filter(([k]) => k !== "cursor"));
  let offset = 0;
  const cursor = q.get("cursor");
  if (cursor) {
    try {
      const c = JSON.parse(atob(cursor));
      if (JSON.stringify(c.filters) !== JSON.stringify(filters)) return bad("cursor");
      offset = c.offset;
    } catch { return bad("cursor"); }
  }
  const page = all.slice(offset, offset + limit);
  return json({ items: page.map(visible), nextCursor: offset + limit < all.length
    ? btoa(JSON.stringify({ offset: offset + limit, filters })) : null });
}
async function comments(req: Request, s: Store, t: Task, u: User) {
  if (req.method === "GET") return json(s.comments.get(t.id));
  const b = await body(req);
  if (!b || typeof b.body !== "string" || !b.body.trim() || b.body.length > 2000) return bad("body");
  const c = { id: s.nextComment++, authorId: u.id, body: b.body, createdAt: stamp(s) };
  s.comments.get(t.id)!.push(c); return json(c, 201);
}
async function dependencies(req: Request, s: Store, t: Task, u: User, p: Project, target?: string) {
  const ids = ((t as any).dependsOn ||= []) as number[];
  if (req.method === "GET") {
    const dependsOn = ids.map(id => s.tasks.find(x => x.id === id)?.key)
      .filter((key): key is string => Boolean(key)).sort(keyOrder);
    const blocks = s.tasks.filter(x => !x.deleted &&
      ((x as any).dependsOn || []).includes(t.id)).map(x => x.key).sort(keyOrder);
    return json({ key: t.key, dependsOn, blocks });
  }
  if (req.method === "DELETE" && target) {
    const other = task(s, target); if (!other || !ids.includes(other.id)) return error("not_found", "not found", 404);
    (t as any).dependsOn = ids.filter(id => id !== other.id); return new Response(null, { status: 204 });
  }
  if (role(p, u) === "viewer") return error("forbidden", "forbidden", 403);
  const b = await body(req), other = b && task(s, b.dependsOn);
  if (!other || other.projectId !== p.id || other.id === t.id) return bad("dependsOn");
  if (ids.includes(other.id)) return error("conflict", "dependency exists", 409);
  const reaches = (from: Task, seen = new Set<number>()): boolean => {
    if (from.id === t.id) return true;
    if (seen.has(from.id)) return false; seen.add(from.id);
    return dependenciesFor(s, from).some(x => reaches(x, seen));
  };
  if (reaches(other)) return error("dependency_cycle", "dependency cycle", 409);
  ids.push(other.id); return json({ key: t.key, dependsOn: ids.map(id => s.tasks.find(x => x.id === id)!.key)
    .sort(keyOrder), blocks: [] }, 201);
}
function keyOrder(a: string, b: string) { return Number(a.split("-")[1]) - Number(b.split("-")[1]); }
function validate(b: any, p: Project, s: Store, prefix = ""): any {
  const field = (f: string) => prefix ? `${prefix}.${f}` : f;
  if (!b || typeof b !== "object" || Array.isArray(b)) return bad(field("title"));
  if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200) return bad(field("title"));
  if (b.description !== undefined && typeof b.description !== "string") return bad(field("description"));
  if (b.priority !== undefined && !["low", "normal", "high"].includes(b.priority)) return bad(field("priority"));
  if (b.labels !== undefined && (!Array.isArray(b.labels) || b.labels.some((x: any) =>
    typeof x !== "string" || !x.trim() || x.trim().length > 30 || /\s/.test(x.trim()))))
    return bad(field("labels"));
  if (b.assigneeId !== undefined && b.assigneeId !== null &&
      (!Number.isInteger(b.assigneeId) || !s.users.some(x => x.id === b.assigneeId) ||
       (b.assigneeId !== p.ownerId && !p.members.has(b.assigneeId)))) return bad(field("assigneeId"));
  return { title: b.title.trim(), description: b.description || "", priority: b.priority || "normal",
    labels: [...new Set((b.labels || []).map((x: string) => x.trim().toLowerCase()))].sort(),
    assigneeId: b.assigneeId ?? null };
}
function makeTask(s: Store, p: Project, u: User, v: any): Task {
  const n = p.next++;
  const t: Task = { id: s.tasks.length + 1, key: `${p.key}-${n}`, number: n, projectId: p.id,
    ...v, status: "open", createdBy: u.id, createdAt: stamp(s), updatedAt: stamp(s), version: 1 };
  s.tasks.push(t); s.history.set(t.id, []); s.comments.set(t.id, []); return visible(t) as Task;
}
async function create(req: Request, s: Store, p: Project, u: User) {
  const r = role(p, u); if (r === "viewer") return error("forbidden", "forbidden", 403);
  const b = await body(req); if (!b) return bad("body");
  const idem = req.headers.get("Idempotency-Key");
  const key = idem ? `${u.id}:${idem}` : "";
  const canonical = JSON.stringify(b);
  const saved = key && s.idempotency.get(key);
  if (saved && s.now() - saved.at < 86400000) {
    if (saved.body !== canonical) return error("idempotency_mismatch", "idempotency key mismatch", 422);
    return json(saved.result, 201, { ETag: `"${saved.result.version}"`, "Idempotent-Replayed": "true" });
  }
  const v = validate(b, p, s); if (v instanceof Response) return v;
  const result = makeTask(s, p, u, v);
  if (key) s.idempotency.set(key, { at: s.now(), body: canonical, result });
  return json(result, 201, { ETag: '"1"' });
}
async function patch(req: Request, s: Store, t: Task, u: User) {
  const p = s.projects.find(x => x.id === t.projectId)!;
  if (role(p, u) === "viewer") return error("forbidden", "forbidden", 403);
  const stale = precondition(req, t); if (stale) return stale;
  const b = await body(req); if (!b) return bad("body");
  const allowed = ["title", "description", "priority", "labels", "assigneeId"];
  for (const k of Object.keys(b)) if (!allowed.includes(k)) return bad(k);
  const v = validate({ ...t, ...b }, p, s); if (v instanceof Response) return v;
  let changed = false;
  for (const k of [...allowed].sort()) if (k in b && JSON.stringify((t as any)[k]) !== JSON.stringify(v[k])) {
    const from = (t as any)[k], to = v[k]; (t as any)[k] = to; changed = true;
    s.history.get(t.id)!.push({ at: stamp(s), by: u.id, field: k, from, to });
  }
  if (changed) { t.version++; t.updatedAt = stamp(s); }
  return json(visible(t), 200, { ETag: `"${t.version}"` });
}
async function transition(req: Request, s: Store, t: Task, u: User, p: Project) {
  if (role(p, u) === "viewer") return error("forbidden", "forbidden", 403);
  const stale = precondition(req, t); if (stale) return stale;
  const b = await body(req); if (!b) return bad("body");
  if (!["open", "in_progress", "done"].includes(b.to)) return bad("to");
  const valid = t.status === "open" && b.to === "in_progress" ||
    t.status === "in_progress" && ["open", "done"].includes(b.to) ||
    t.status === "done" && b.to === "open";
  if (!valid) return error("invalid_transition", "invalid transition", 409);
  if (b.to === "done" && dependenciesFor(s, t).some(x => x.status !== "done"))
    return error("blocked", "task is blocked", 409);
  s.history.get(t.id)!.push({ at: stamp(s), by: u.id, field: "status", from: t.status, to: b.to });
  t.status = b.to; t.version++; t.updatedAt = stamp(s);
  return json(visible(t), 200, { ETag: `"${t.version}"` });
}
