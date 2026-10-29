import { auth, json } from "./response";
import { iso, member, taskByKey, validate, type Ctx, type Task } from "./model";
import { dependencyRoute } from "./dependencies";
function fields(c: Ctx, task?: Task, prefix = "") {
  const b = c.body, allowed = ["title", "description", "priority", "labels", "assigneeId"];
  if (task) for (const k of Object.keys(b)) validate(allowed.includes(k), prefix + k);
  const out: any = {};
  if ("title" in b || !task) {
    const v = typeof b.title === "string" ? b.title.trim() : "";
    validate(v.length > 0 && v.length <= 200, prefix + "title"); out.title = v;
  }
  if ("description" in b || !task) {
    const v = b.description ?? ""; validate(typeof v === "string", prefix + "description");
    out.description = v;
  }
  if ("priority" in b || !task) {
    const v = b.priority ?? "normal";
    validate(["low", "normal", "high"].includes(v), prefix + "priority"); out.priority = v;
  }
  if ("labels" in b || !task) {
    const v = b.labels ?? [];
    validate(Array.isArray(v) && v.every((x: any) =>
      typeof x === "string" && x.trim().length <= 30 && !/\s/.test(x.trim())),
    prefix + "labels");
    out.labels = [...new Set(v.map((x: string) => x.trim().toLowerCase()))].sort();
  }
  if ("assigneeId" in b || !task) {
    const id = b.assigneeId ?? null;
    const projectId = task?.projectId || Number(c.url.pathname.match(/projects\/(\d+)/)?.[1]);
    const project = c.s.projects.find(p => p.id === projectId);
    const isMember = c.s.memberships.get(projectId)?.has(id) || project?.ownerId === id;
    validate(id === null || (Number.isInteger(id) && isMember), prefix + "assigneeId");
    out.assigneeId = id;
  }
  return out;
}
export function createTask(c: Ctx, p: any, body = c.body, fieldPrefix = "") {
  c.body = body; const f = fields(c, undefined, fieldPrefix); c.body = body;
  const seqKey = `task${p.id}`;
  const number = (c.s.ids[seqKey] = (c.s.ids[seqKey] || 0) + 1);
  const id = (c.s.ids.task = c.s.ids.task! + 1);
  const t: Task = { id, key: `${p.key}-${number}`, number, projectId: p.id,
    ...f, status: "open", createdBy: c.user!.id, createdAt: iso(c.s), updatedAt: iso(c.s), version: 1 };
  c.s.tasks.push(t); c.s.comments.set(t.id, []); c.s.history.set(t.id, []); return t;
}
export function taskRoute(c: Ctx, path: string) {
  const { s } = c;
  const pm = path.match(/^\/projects\/(\d+)\/(tasks|stats)(?:\/(bulk))?$/);
  if (pm) {
    auth(c); const p = s.projects.find(x => x.id === +pm[1]!);
    if (!p || !member(s, p, c.user!)) throw Object.assign(Error("Not found"), { status: 404, code: "not_found" });
    const role = member(s, p, c.user!);
    if (pm[2] === "stats" && c.req.method === "GET") {
      const ts = s.tasks.filter(t => t.projectId === p.id);
      const statuses = ["open", "in_progress", "done"];
      const priorities = ["high", "normal", "low"];
      const counts = (key: string, vals: string[]) => Object.fromEntries(
        vals.map(v => [v, ts.filter(t => (t as any)[key] === v).length]),
      );
      const assignees = new Map<number, number>();
      ts.forEach(t => {
        if (t.assigneeId)
          assignees.set(t.assigneeId, (assignees.get(t.assigneeId) || 0) + 1);
      });
      return json({
        total: ts.length, byStatus: counts("status", statuses), byPriority: counts("priority", priorities),
        byAssignee: [...assignees].sort((a,b) => b[1]-a[1] || a[0]-b[0])
          .map(([userId,count]) => ({ userId,count })),
      });
    }
    if (pm[2] === "tasks" && c.req.method === "POST") {
      if (role === "viewer") throw Object.assign(Error("Forbidden"), { status: 403, code: "forbidden" });
      if (pm[3] === "bulk") {
        validate(Array.isArray(c.body.tasks) && c.body.tasks.length >= 1 && c.body.tasks.length <= 50, "tasks");
        c.body.tasks.forEach((b: any, i: number) => fields({ ...c, body: b }, undefined, `tasks[${i}].`));
        const staged: Task[] = [];
        c.body.tasks.forEach((b: any) => staged.push(createTask({ ...c, body: b }, p, b)));
        return json({ items: staged }, 201);
      }
      const idempotency = c.req.headers.get("Idempotency-Key");
      const cacheKey = `${c.user!.id}:${idempotency || ""}`;
      const prior = idempotency && s.idem.get(cacheKey);
      if (prior && s.now() - prior.at < 86400000) {
        if (prior.body !== JSON.stringify(c.body))
          throw Object.assign(Error("Idempotency key reused"), { status: 422, code: "idempotency_mismatch" });
        const replay = json(prior.task, 201, { ETag: '"1"', "Idempotent-Replayed": "true" });
        return replay;
      }
      const t = createTask(c, p);
      if (idempotency) s.idem.set(cacheKey, { body: JSON.stringify(c.body), task: t, at: s.now() });
      return json(t, 201, { ETag: '"1"' });
    }
    if (pm[2] === "tasks" && c.req.method === "GET") return listTasks(c, p);
  }
  const match = path.match(/^\/tasks\/([^/]+)(?:\/(.*))?$/);
  if (!match) return;
  auth(c); const t = taskByKey(s, decodeURIComponent(match[1]!));
  const p = t && s.projects.find(x => x.id === t.projectId);
  if (!t || !p || !member(s, p, c.user!)) throw Object.assign(Error("Not found"), { status: 404, code: "not_found" });
  const tail = match[2] || "";
  const dependency = tail.match(/^dependencies(?:\/(.+))?$/);
  if (dependency) return dependencyRoute(c, t, dependency[1]);
  if (!tail && c.req.method === "GET") return json(t, 200, { ETag: `"${t.version}"` });
  if (!tail && c.req.method === "DELETE") {
    if (member(s,p,c.user!) === "viewer" || (member(s,p,c.user!) !== "owner" && t.createdBy !== c.user!.id))
      throw Object.assign(Error("Forbidden"), { status: 403, code: "forbidden" });
    s.tasks = s.tasks.filter(x => x !== t);
    s.deps.delete(t.id);
    for (const set of s.deps.values()) set.delete(t.id);
    return new Response(null, { status: 204 });
  }
  if (!tail && c.req.method === "PATCH") return patchTask(c, t, p);
  if (tail === "transitions" && c.req.method === "POST") return transition(c, t);
  if (tail === "history" && c.req.method === "GET") return json(s.history.get(t.id));
  if (tail === "comments") return comments(c, t);
}
function listTasks(c: Ctx, p: any) {
  const q = c.url.searchParams, status = q.get("status"), sort = q.get("sort") || "created";
  const limit = Number(q.get("limit") || 20);
  validate(!status || ["open","in_progress","done"].includes(status), "status");
  validate(["created","-created","priority"].includes(sort), "sort");
  validate(Number.isInteger(limit) && limit >= 1 && limit <= 100, "limit");
  let ts = c.s.tasks.filter(t => t.projectId === p.id && (!status || t.status === status));
  const a = q.get("assignee");
  if (a) {
    const id = a === "me" ? c.user!.id : a === "none" ? null : Number(a);
    validate(a === "me" || a === "none" || (Number.isInteger(id) && id! > 0), "assignee");
    ts = ts.filter(t => t.assigneeId === id);
  }
  if (q.has("label")) ts = ts.filter(t => t.labels.includes(q.get("label")!.toLowerCase()));
  if (q.has("q")) {
    const term = q.get("q")!.toLowerCase();
    ts = ts.filter(t => `${t.title} ${t.description}`.toLowerCase().includes(term));
  }
  ts.sort((a,b) => sort === "-created" ? b.number-a.number :
    sort === "priority" ? priority(a) - priority(b) || a.number-b.number : a.number-b.number);
  let start = 0;
  if (q.has("cursor")) {
    try {
      start = Number(atob(q.get("cursor")!));
      validate(Number.isInteger(start), "cursor");
    } catch { validate(false, "cursor"); }
  }
  const items = ts.slice(start, start + limit);
  const nextCursor = start + limit < ts.length ? btoa(String(start + limit)) : null;
  return json({ items, nextCursor });
}
function record(c: Ctx, t: Task, field: string, from: any, to: any) {
  c.s.history.get(t.id)!.push({ at: iso(c.s), by: c.user!.id, field, from, to });
}
function patchTask(c: Ctx, t: Task, p: any) {
  if (member(c.s,p,c.user!) === "viewer") throw Object.assign(Error("Forbidden"), { status: 403, code: "forbidden" });
  const expected = c.req.headers.get("If-Match");
  if (expected && expected !== `"${t.version}"`)
    throw Object.assign(Error("Version changed"), { status: 412, code: "precondition_failed" });
  const next = fields(c, t);
  let changed = false;
  for (const key of Object.keys(next).sort()) {
    const from = (t as any)[key], to = next[key];
    if (JSON.stringify(from) !== JSON.stringify(to)) {
      record(c, t, key, from, to); (t as any)[key] = to; changed = true;
    }
  }
  if (changed) { t.version++; t.updatedAt = iso(c.s); }
  return json(t, 200, { ETag: `"${t.version}"` });
}
function transition(c: Ctx, t: Task) {
  const role = member(c.s, c.s.projects.find(p => p.id === t.projectId)!, c.user!);
  if (role === "viewer") throw Object.assign(Error("Forbidden"), { status: 403, code: "forbidden" });
  const to = c.body.to; validate(["open","in_progress","done"].includes(to), "to");
  const expected = c.req.headers.get("If-Match");
  if (expected && expected !== `"${t.version}"`)
    throw Object.assign(Error("Version changed"), { status: 412, code: "precondition_failed" });
  const allowed: Record<string, string[]> = { open: ["in_progress"], in_progress: ["open","done"], done: ["open"] };
  if (!allowed[t.status]?.includes(to))
    throw Object.assign(Error("Invalid transition"), { status: 409, code: "invalid_transition" });
  if (to === "done" && [...(c.s.deps.get(t.id) || [])].some(id =>
    c.s.tasks.find(task => task.id === id)?.status !== "done"))
    throw Object.assign(Error("Dependencies are incomplete"), { status: 409, code: "blocked" });
  record(c,t,"status",t.status,to); t.status = to; t.version++; t.updatedAt = iso(c.s);
  return json(t, 200, { ETag: `"${t.version}"` });
}
function comments(c: Ctx, t: Task) {
  const items = c.s.comments.get(t.id)!;
  if (c.req.method === "GET") return json(items);
  validate(typeof c.body.body === "string" && c.body.body.trim().length > 0 && c.body.body.length <= 2000, "body");
  const item = {
    id: (c.s.ids.comment = c.s.ids.comment! + 1), authorId: c.user!.id,
    body: c.body.body.trim(), createdAt: iso(c.s),
  };
  items.push(item); return json(item, 201);
}
function priority(task: Task) {
  return ["high", "normal", "low"].indexOf(task.priority);
}
