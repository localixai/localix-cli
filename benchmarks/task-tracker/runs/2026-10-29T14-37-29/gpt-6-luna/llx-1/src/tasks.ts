import { body, err, json, match, member, projectFor, taskByKey, taskResponse, type Ctx } from "./http";
import { pair, type Task } from "./model";
function validate(b: any, partial = false): string | null {
  if (!partial && (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200)) return "title";
  if ("title" in b && (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200)) return "title";
  if ("description" in b && typeof b.description !== "string") return "description";
  if ("priority" in b && !["low", "normal", "high"].includes(b.priority)) return "priority";
  if ("labels" in b && (!Array.isArray(b.labels) || b.labels.some((x: any) =>
    typeof x !== "string" || !x.trim() || x.trim().length > 30 || /\s/.test(x.trim())))) return "labels";
  return null;
}
function create(c: Ctx, p: any, b: any): Task {
  const n = p.next++, labels = [...new Set<string>((b.labels || []).map((x: string) => x.trim().toLowerCase()))].sort();
  const t: Task = { id: c.s.tasks.length + 1, key: `${p.key}-${n}`, number: n, projectId: p.id,
    title: b.title.trim(), description: b.description || "", status: "open",
    priority: b.priority || "normal", labels, assigneeId: b.assigneeId ?? null,
    createdBy: c.user!, createdAt: c.now, updatedAt: c.now, version: 1 };
  c.s.tasks.push(t); return t;
}
export async function taskRoutes(c: Ctx, path: string) {
  let m = path.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if (m) {
    const id = Number(m[1]), p = projectFor(c, id);
    if (!p) return err("not_found", "Not found", undefined, 404);
    if (!member(c, id, c.req.method === "POST")) return err("forbidden", "Forbidden", undefined, 403);
    if (c.req.method === "POST") {
      const b = await body(c); if (!b) return err("validation", "Invalid JSON");
      const bulk = Boolean(m[2]);
      if (!bulk) {
        const e = validate(b); if (e) return err("validation", `Invalid ${e}`, e);
        if (b.assigneeId != null && !c.s.members.has(pair(id, b.assigneeId))) return err("validation", "Invalid assignee", "assigneeId");
        return taskResponse(create(c, p, b), 201);
      }
      if (!Array.isArray(b.tasks) || !b.tasks.length || b.tasks.length > 50) return err("validation", "Invalid tasks", "tasks");
      for (let i = 0; i < b.tasks.length; i++) {
        const e = validate(b.tasks[i]); if (e) return err("validation", `Invalid ${e}`, `tasks[${i}].${e}`);
        if (b.tasks[i].assigneeId != null && !c.s.members.has(pair(id, b.tasks[i].assigneeId)))
          return err("validation", "Invalid assignee", `tasks[${i}].assigneeId`);
      }
      return json({ items: b.tasks.map((x: any) => create(c, p, x)) }, 201);
    }
    if (c.req.method === "GET") return list(c, id);
  }
  const t = taskByKey(c); if (!t) return null;
  if (path.endsWith("/comments")) return comments(c, t);
  if (path.endsWith("/history") && c.req.method === "GET") return json(c.s.history.get(t.id) || []);
  if (path.endsWith("/dependencies")) return dependencies(c, t);
  if (path.endsWith("/transitions")) return transition(c, t);
  if (path === `/tasks/${t.key}` && c.req.method === "GET") return taskResponse(t);
  if (path === `/tasks/${t.key}` && c.req.method === "DELETE") {
    if (!member(c, t.projectId, true)) return err("forbidden", "Forbidden", undefined, 403);
    if (t.createdBy !== c.user && t.createdBy !== t.projectId && c.s.members.get(pair(t.projectId, c.user!)) !== "owner")
      return err("forbidden", "Forbidden", undefined, 403);
    t.deleted = true; c.s.deps.delete(t.id); c.s.deps.forEach(d => d.delete(t.id));
    return new Response(null, { status: 204 });
  }
  if (path === `/tasks/${t.key}` && c.req.method === "PATCH") return patch(c, t);
  return null;
}
async function patch(c: Ctx, t: Task) {
  if (!member(c, t.projectId, true)) return err("forbidden", "Forbidden", undefined, 403);
  if (!match(c, t.version)) return err("precondition_failed", "Version changed", undefined, 412);
  const b = await body(c); if (!b) return err("validation", "Invalid JSON");
  for (const k of Object.keys(b)) if (!["title", "description", "priority", "labels", "assigneeId"].includes(k))
    return err("validation", `Invalid ${k}`, k);
  const e = validate(b, true); if (e) return err("validation", `Invalid ${e}`, e);
  if (b.assigneeId != null && !c.s.members.has(pair(t.projectId, b.assigneeId))) return err("validation", "Invalid assignee", "assigneeId");
  const before = { ...t }, fields = ["assigneeId", "description", "labels", "priority", "title"];
  const changes = fields.filter(k => k in b && JSON.stringify((t as any)[k]) !== JSON.stringify(k === "labels" ?
    [...new Set<string>(b[k].map((x: string) => x.trim().toLowerCase()))].sort() : k === "title" ? b[k].trim() : b[k]));
  changes.forEach(k => {
    const value = k === "labels" ? [...new Set<string>(b[k].map((x: string) => x.trim().toLowerCase()))].sort() :
      k === "title" ? b[k].trim() : b[k];
    (t as any)[k] = value;
    (c.s.history.get(t.id) || (c.s.history.set(t.id, []), c.s.history.get(t.id)!))
      .push({ at: c.now, by: c.user!, field: k, from: (before as any)[k], to: value });
  });
  if (changes.length) { t.version++; t.updatedAt = c.now; }
  return taskResponse(t);
}
function list(c: Ctx, id: number) {
  let ts = c.s.tasks.filter(t => t.projectId === id && !t.deleted);
  const q = c.url.searchParams;
  if (q.has("status")) ts = ts.filter(t => t.status === q.get("status"));
  if (q.has("label")) ts = ts.filter(t => t.labels.includes(q.get("label")!.toLowerCase()));
  if (q.has("q")) ts = ts.filter(t => `${t.title} ${t.description}`.toLowerCase().includes(q.get("q")!.toLowerCase()));
  if (q.has("assignee")) ts = ts.filter(t => q.get("assignee") === "none" ? !t.assigneeId :
    q.get("assignee") === "me" ? t.assigneeId === c.user : t.assigneeId === Number(q.get("assignee")));
  if (q.get("sort") === "-created") ts.reverse();
  if (q.get("sort") === "priority") ts.sort((a, b) =>
    ["high", "normal", "low"].indexOf(a.priority) - ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number);
  let limit = Number(q.get("limit") || 20);
  if (!Number.isInteger(limit) || limit < 1 || limit > 100) return err("validation", "Invalid limit", "limit");
  if (q.has("status") && !["open", "in_progress", "done"].includes(q.get("status")!))
    return err("validation", "Invalid status", "status");
  if (q.has("sort") && !["-created", "priority"].includes(q.get("sort")!))
    return err("validation", "Invalid sort", "sort");
  if (q.has("assignee") && !["none", "me"].includes(q.get("assignee")!) &&
    !/^\d+$/.test(q.get("assignee")!)) return err("validation", "Invalid assignee", "assignee");
  if (q.has("cursor") && !/^\d+$/.test(atobSafe(q.get("cursor")!)))
    return err("validation", "Invalid cursor", "cursor");
  const cursor = q.get("cursor") ? Number(atobSafe(q.get("cursor")!)) : 0;
  if (!Number.isInteger(cursor) || cursor < 0) return err("validation", "Invalid cursor", "cursor");
  const offset = cursor;
  const items = ts.slice(offset, offset + limit);
  return json({ items, nextCursor: offset + limit < ts.length ? btoa(String(offset + limit)) : null });
}
function atobSafe(value: string) { try { return atob(value); } catch { return ""; } }
async function transition(c: Ctx, t: Task) {
  if (c.req.method !== "POST") return err("method_not_allowed", "Method not allowed", undefined, 405);
  if (!member(c, t.projectId, true)) return err("forbidden", "Forbidden", undefined, 403);
  if (!match(c, t.version)) return err("precondition_failed", "Version changed", undefined, 412);
  const b = await body(c);
  if (!["open", "in_progress", "done"].includes(b?.to)) return err("validation", "Invalid status", "to");
  const allowed: Record<string, string[]> = { open: ["in_progress"], in_progress: ["open", "done"], done: ["open"] };
  if (!allowed[t.status]!.includes(b.to)) return err("invalid_transition", "Invalid transition", undefined, 409);
  if (b.to === "done" && [...(c.s.deps.get(t.id) || [])].some(id => c.s.tasks.find(x => x.id === id)?.status !== "done"))
    return err("blocked", "Blocked by dependency", undefined, 409);
  const from = t.status; t.status = b.to; t.version++; t.updatedAt = c.now;
  (c.s.history.get(t.id) || (c.s.history.set(t.id, []), c.s.history.get(t.id)!))
    .push({ at: c.now, by: c.user!, field: "status", from, to: b.to });
  return taskResponse(t);
}
async function comments(c: Ctx, t: Task) {
  const a = c.s.comments.get(String(t.id)) || [];
  if (c.req.method === "GET") return json(a);
  const b = await body(c), text = b?.body;
  if (typeof text !== "string" || !text.trim() || text.length > 2000) return err("validation", "Invalid body", "body");
  const item = { id: a.length + 1, authorId: c.user, body: text, createdAt: c.now };
  a.push(item); c.s.comments.set(String(t.id), a); return json(item, 201);
}
async function dependencies(c: Ctx, t: Task) {
  const all = () => [...(c.s.deps.get(t.id) || [])].map(id => c.s.tasks.find(x => x.id === id)!)
    .filter(Boolean).sort((a, b) => a.number - b.number).map(x => x.key);
  const blocks = () => c.s.tasks.filter(x => !x.deleted && c.s.deps.get(x.id)?.has(t.id))
    .sort((a, b) => a.number - b.number).map(x => x.key);
  if (c.req.method === "GET") return json({ key: t.key, dependsOn: all(), blocks: blocks() });
  const target = c.url.pathname.split("/")[4];
  if (c.req.method === "DELETE") {
    const other = taskByKey({ ...c, url: new URL(`http://x/tasks/${target}`) });
    if (!other || !c.s.deps.get(t.id)?.has(other.id)) return err("not_found", "Not found", undefined, 404);
    c.s.deps.get(t.id)!.delete(other.id); return new Response(null, { status: 204 });
  }
  if (!member(c, t.projectId, true)) return err("forbidden", "Forbidden", undefined, 403);
  const b = await body(c), other = typeof b?.dependsOn === "string" ? c.s.tasks.find(x => x.key === b.dependsOn && !x.deleted) : null;
  if (!other || other.projectId !== t.projectId || other.id === t.id) return err("validation", "Invalid dependency", "dependsOn");
  const deps = c.s.deps.get(t.id) || new Set<number>();
  if (deps.has(other.id)) return err("conflict", "Dependency exists", undefined, 409);
  const reaches = (from: number, targetId: number): boolean => from === targetId ||
    [...(c.s.deps.get(from) || [])].some(x => reaches(x, targetId));
  if (reaches(other.id, t.id)) return err("dependency_cycle", "Dependency cycle", undefined, 409);
  deps.add(other.id); c.s.deps.set(t.id, deps);
  return json({ key: t.key, dependsOn: all(), blocks: blocks() }, 201);
}
