import { fail, labels, ok, projectAccess, taskAccess, type Ctx, type Result } from "./http";
import { bad, iso, taskView, type Task } from "./model";

function validate(b: any, p: any, s: Ctx["state"], partial = false): any {
  if (!partial || "title" in b) {
    if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200) return "title";
  }
  if ("description" in b && typeof b.description !== "string") return "description";
  if ("priority" in b && !["low", "normal", "high"].includes(b.priority)) return "priority";
  if ("labels" in b && labels(b.labels) === null) return "labels";
  if ("assigneeId" in b && b.assigneeId !== null &&
      (!Number.isInteger(b.assigneeId) || !p.members.has(b.assigneeId))) return "assigneeId";
}
function create(c: Ctx, p: any, b: any): Task {
  const n = ++p.next, now = iso(c.state);
  const t: Task = { id: ++c.state.ids.task, key: `${p.key}-${n}`, number: n,
    projectId: p.id, title: b.title.trim(), description: b.description ?? "",
    status: "open", priority: b.priority ?? "normal", labels: labels(b.labels ?? [])!,
    assigneeId: b.assigneeId ?? null, createdBy: c.user!, createdAt: now, updatedAt: now,
    version: 1, deleted: false, history: [], comments: [], depends: new Set() };
  c.state.tasks.push(t); return t;
}
function etag(t: Task) { return { ETag: `"${t.version}"` }; }
export function taskRoutes(c: Ctx, path: string): Result | undefined {
  const s = c.state;
  let m = path.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if (m) {
    const p = projectAccess(c, Number(m[1]));
    if (!p) return fail(404, "not_found");
    if (c.req.method === "POST") {
      if (p.members.get(c.user!) === "viewer") return fail(403, "forbidden");
      if (m[2]) {
        const list = c.body?.tasks;
        if (!Array.isArray(list) || list.length < 1 || list.length > 50)
          return { status: 400, body: bad("tasks") };
        for (let i = 0; i < list.length; i++) {
          const f = validate(list[i] ?? {}, p, s);
          if (f) return { status: 400, body: bad(`tasks[${i}].${f}`) };
        }
        return ok({ items: list.map((b: any) => taskView(create(c, p, b))) }, 201);
      }
      const f = validate(c.body ?? {}, p, s);
      if (f) return { status: 400, body: bad(f) };
      const idem = c.req.headers.get("Idempotency-Key");
      const ik = `${c.user}:${idem}`;
      const old = idem && s.idem.get(ik);
      const serialized = JSON.stringify(c.body);
      if (old && s.now() - old.at < 86400000) {
        if (old.body !== serialized) return fail(422, "idempotency_mismatch");
        return ok(taskView(old.task), 201, { "Idempotent-Replayed": "true", ...etag(old.task) });
      }
      const t = create(c, p, c.body);
      if (idem) s.idem.set(ik, { at: s.now(), body: serialized, task: t });
      return ok(taskView(t), 201, etag(t));
    }
    if (c.req.method === "GET") return listTasks(c, p);
  }
  m = path.match(/^\/tasks\/([A-Z]+-\d+)(?:\/(.*))?$/);
  if (!m) return;
  const t = taskAccess(c, m[1]!);
  if (!t) return fail(404, "not_found");
  const p = s.projects.find(x => x.id === t.projectId)!;
  const sub = m[2] ?? "";
  if (sub === "" && c.req.method === "GET") return ok(taskView(t), 200, etag(t));
  if (sub === "" && c.req.method === "DELETE") {
    if (p.members.get(c.user!) === "viewer" ||
        (p.ownerId !== c.user && t.createdBy !== c.user)) return fail(403, "forbidden");
    t.deleted = true;
    for (const x of s.tasks) x.depends.delete(t.key);
    return { status: 204 };
  }
  if (sub === "" && c.req.method === "PATCH") {
    if (p.members.get(c.user!) === "viewer") return fail(403, "forbidden");
    const f = validate(c.body ?? {}, p, s, true);
    if (f) return { status: 400, body: bad(f) };
    const allowed = ["title", "description", "priority", "labels", "assigneeId"];
    const unknown = Object.keys(c.body ?? {}).find(k => !allowed.includes(k));
    if (unknown) return { status: 400, body: bad(unknown) };
    const pre = checkMatch(c, t); if (pre) return pre;
    const fields = ["assigneeId", "description", "labels", "priority", "title"];
    let changed = false;
    for (const field of fields) if (field in c.body) {
      const v = field === "labels" ? labels(c.body[field]) : field === "title" ? c.body[field].trim() : c.body[field];
      if (JSON.stringify((t as any)[field]) !== JSON.stringify(v)) {
        t.history.push({ at: iso(s), by: c.user, field, from: (t as any)[field], to: v });
        (t as any)[field] = v; t.updatedAt = iso(s); changed = true;
      }
    }
    if (changed) t.version++;
    return ok(taskView(t), 200, etag(t));
  }
  return taskSubroutes(c, t, p, sub);
}
function checkMatch(c: Ctx, t: Task): Result | undefined {
  const h = c.req.headers.get("If-Match");
  return h && h !== `"${t.version}"` ? fail(412, "precondition_failed") : undefined;
}
function taskSubroutes(c: Ctx, t: Task, p: any, sub: string): Result | undefined {
  const { state: s, user } = c;
  if (sub === "history" && c.req.method === "GET") return ok(t.history);
  if (sub === "comments" && c.req.method === "GET") return ok(t.comments);
  if (sub === "comments" && c.req.method === "POST") {
    const body = c.body?.body;
    if (typeof body !== "string" || !body.trim() || body.length > 2000)
      return { status: 400, body: bad("body") };
    const item = { id: ++s.ids.comment, authorId: user, body: body.trim(), createdAt: iso(s) };
    t.comments.push(item); return ok(item, 201);
  }
  if (sub === "transitions" && c.req.method === "POST") {
    if (p.members.get(user!) === "viewer") return fail(403, "forbidden");
    const to = c.body?.to;
    if (!["open", "in_progress", "done"].includes(to)) return { status: 400, body: bad("to") };
    const pre = checkMatch(c, t); if (pre) return pre;
    const allowed = t.status === "open" ? to === "in_progress" :
      t.status === "in_progress" ? ["open", "done"].includes(to) : to === "open";
    if (!allowed) return fail(409, "invalid_transition");
    if (to === "done" && [...t.depends].some(k => {
      const d = s.tasks.find(x => x.key === k && !x.deleted); return d && d.status !== "done";
    })) return fail(409, "blocked");
    t.history.push({ at: iso(s), by: user, field: "status", from: t.status, to });
    t.status = to; t.version++; t.updatedAt = iso(s);
    return ok(taskView(t), 200, etag(t));
  }
  if (sub === "dependencies" && c.req.method === "GET") {
    const keys = [...t.depends].sort((a, b) => Number(a.split("-")[1]) - Number(b.split("-")[1]));
    const blocks = s.tasks.filter(x => !x.deleted && x.depends.has(t.key))
      .sort((a, b) => a.number - b.number).map(x => x.key);
    return ok({ key: t.key, dependsOn: keys, blocks });
  }
  const dm = sub.match(/^dependencies(?:\/([A-Z]+-\d+))?$/);
  if (dm && c.req.method === "POST" && !dm[1]) {
    if (p.members.get(user!) === "viewer") return fail(403, "forbidden");
    const key = c.body?.dependsOn as string;
    const d = s.tasks.find(x => x.key === key && !x.deleted && x.projectId === t.projectId);
    if (!d || d === t) return { status: 400, body: bad("dependsOn") };
    if (t.depends.has(key)) return fail(409, "conflict");
    const reaches = (x: Task, seen = new Set<string>()): boolean => {
      if (x.key === t.key) return true;
      if (seen.has(x.key)) return false;
      seen.add(x.key);
      return [...x.depends].some(k => {
        const next = s.tasks.find(y => y.key === k && !y.deleted);
        return !!next && reaches(next, seen);
      });
    };
    if (reaches(d)) return fail(409, "dependency_cycle");
    t.depends.add(key);
    const blocks = s.tasks.filter(x => x.depends.has(t.key)).map(x => x.key);
    return ok({ key: t.key, dependsOn: [...t.depends], blocks }, 201);
  }
  const del = sub.match(/^dependencies\/([A-Z]+-\d+)$/);
  if (del && c.req.method === "DELETE") {
    if (!t.depends.delete(del[1]!)) return fail(404, "not_found");
    return { status: 204 };
  }
}
function listTasks(c: Ctx, p: any): Result {
  const q = c.url.searchParams, s = c.state;
  let ts = s.tasks.filter(t => t.projectId === p.id && !t.deleted);
  const status = q.get("status"), assignee = q.get("assignee"), label = q.get("label");
  const search = q.get("q")?.toLowerCase();
  if (status && !["open", "in_progress", "done"].includes(status))
    return { status: 400, body: bad("status") };
  if (status) ts = ts.filter(t => t.status === status);
  if (assignee) {
    if (assignee === "none") ts = ts.filter(t => !t.assigneeId);
    else if (assignee === "me") ts = ts.filter(t => t.assigneeId === c.user);
    else if (/^\d+$/.test(assignee)) ts = ts.filter(t => t.assigneeId === Number(assignee));
    else return { status: 400, body: bad("assignee") };
  }
  if (label) ts = ts.filter(t => t.labels.includes(label.toLowerCase()));
  if (search) ts = ts.filter(t => `${t.title} ${t.description}`.toLowerCase().includes(search));
  const sort = q.get("sort") ?? "created";
  if (!["created", "-created", "priority"].includes(sort)) return { status: 400, body: bad("sort") };
  const rank: any = { high: 0, normal: 1, low: 2 };
  ts.sort((a, b) => sort === "priority" ? rank[a.priority] - rank[b.priority] || a.number - b.number :
    sort === "-created" ? b.number - a.number : a.number - b.number);
  const rawLimit = q.get("limit") ?? "20", limit = Number(rawLimit);
  if (!/^\d+$/.test(rawLimit) || limit < 1 || limit > 100)
    return { status: 400, body: bad("limit") };
  let offset = 0;
  const cursor = q.get("cursor");
  if (cursor) {
    try { offset = Number(JSON.parse(atob(cursor)).o); } catch { return { status: 400, body: bad("cursor") }; }
    if (!Number.isInteger(offset) || offset < 0) return { status: 400, body: bad("cursor") };
  }
  const items = ts.slice(offset, offset + limit);
  const nextCursor = offset + limit < ts.length ? btoa(JSON.stringify({ o: offset + limit })) : null;
  return ok({ items: items.map(taskView), nextCursor });
}
