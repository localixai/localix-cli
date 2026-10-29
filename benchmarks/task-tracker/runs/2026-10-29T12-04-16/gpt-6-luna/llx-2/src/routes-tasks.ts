import { Ctx, Result, auth, member, validation } from "./routes";
import { fail, Fault } from "./http";
import { Task } from "./store";
export async function tasks(c: Ctx): Promise<Result> {
  const { path, method, db } = c;
  const project = path.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if (project) return projectTasks(c, Number(project[1]), !!project[2]);
  const m = path.match(/^\/tasks\/([A-Z]+-\d+)(?:\/(.*))?$/);
  if (!m) fail(404, "not_found", "Not found");
  const t = db.task(m[1]!), sub = m[2];
  if (!t) fail(404, "not_found", "Task not found");
  member(c, t.projectId);
  if (sub?.startsWith("dependencies")) return dependency(c, t);
  if (sub === "history") return { status: 200, body: t.history };
  if (sub === "comments") return comments(c, t);
  if (sub === "transitions") return transition(c, t);
  if (!sub && method === "GET") return { status: 200, body: clean(t), headers: { ETag: `"${t.version}"` } };
  if (!sub && method === "PATCH") return patch(c, t);
  if (!sub && method === "DELETE") {
    const uid = auth(c), owner = db.projects.find(p => p.id === t.projectId)!.ownerId;
    if (uid !== t.createdBy && uid !== owner) fail(403, "forbidden", "Forbidden");
    db.tasks = db.tasks.filter(x => x !== t);
    db.tasks.forEach(x => x.deps.delete(t.key)); return { status: 204 };
  }
  fail(404, "not_found", "Not found");
}
export function projectTasks(c: Ctx, pid: number, bulk = false): Result {
  member(c, pid, c.method !== "GET");
  if (bulk) {
    if (c.method !== "POST") return methodFail("POST");
    const list = c.body.tasks;
    if (!Array.isArray(list) || list.length < 1 || list.length > 50) validation("tasks");
    const checked = list.map((b: any, i: number) => validate(c, pid, b, `tasks[${i}].`));
    return { status: 201, body: { items: checked.map(b => clean(create(c, pid, b))) } };
  }
  if (c.method === "POST") {
    const key = c.headers.get("Idempotency-Key"), uid = auth(c);
    const existing = key && c.db.idem.get(`${uid}:${key}`);
    if (existing && c.db.now()-existing.time < 86400000) {
      if (existing.body !== JSON.stringify(c.body)) fail(422, "idempotency_mismatch", "Key reused");
      return { status: 201, body: clean(existing.task), headers: { "Idempotent-Replayed": "true",
        ETag: `"${existing.task.version}"` } };
    }
    const fields = validate(c, pid, c.body, "");
    const task = create(c, pid, fields);
    if (key) c.db.idem.set(`${uid}:${key}`, { body: JSON.stringify(c.body), task, time: c.db.now() });
    return { status: 201, body: clean(task), headers: { ETag: '"1"' } };
  }
  if (c.method !== "GET") return methodFail("GET, POST");
  return listing(c, pid);
}
function validate(c: Ctx, pid: number, b: any, prefix: string) {
  if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200)
    validation(prefix+"title");
  if (b.description !== undefined && typeof b.description !== "string") validation(prefix+"description");
  if (b.priority !== undefined && !["low","normal","high"].includes(b.priority))
    validation(prefix+"priority");
  if (b.labels !== undefined && (!Array.isArray(b.labels) ||
    b.labels.some((x: any) => typeof x !== "string" || !x.trim() || x.length > 30 || /\s/.test(x.trim()))))
    validation(prefix+"labels");
  if (b.assigneeId != null && !c.db.role(pid, b.assigneeId)) validation(prefix+"assigneeId");
  return b;
}
function create(c: Ctx, pid: number, b: any): Task {
  const p = c.db.projects.find(x => x.id === pid)!;
  const number = (c.db.projectNumbers.get(pid) ?? 0)+1;
  c.db.projectNumbers.set(pid, number);
  const now = c.db.iso(), labels: string[] =
    [...new Set<string>((b.labels ?? []).map((x: string) => x.trim().toLowerCase()))].sort();
  const t: Task = { id: ++c.db.seq, key: `${p.key}-${number}`, number, projectId: pid,
    title: b.title.trim(), description: b.description ?? "", status: "open", priority: b.priority ?? "normal",
    labels, assigneeId: b.assigneeId ?? null, createdBy: auth(c), createdAt: now, updatedAt: now,
    version: 1, history: [], comments: [], deps: new Set() };
  c.db.tasks.push(t); return t;
}
function clean(t: Task) {
  const { history, comments, deps, ...out } = t; return out;
}
function methodFail(allow: string): never {
  throw new Fault(405, "method_not_allowed", "Method not allowed", undefined, allow);
}
function listing(c: Ctx, pid: number): Result {
  let ts = c.db.tasks.filter(t => t.projectId === pid);
  const q = c.query, status = q.get("status"), assignee = q.get("assignee");
  if (status && !["open", "in_progress", "done"].includes(status)) validation("status");
  if (status) ts = ts.filter(t => t.status === status);
  if (assignee) {
    if (!["me", "none"].includes(assignee) && !/^\d+$/.test(assignee)) validation("assignee");
    const uid = auth(c);
    ts = ts.filter(t => assignee === "none" ? t.assigneeId === null :
      t.assigneeId === (assignee === "me" ? uid : Number(assignee)));
  }
  const label = q.get("label")?.toLowerCase();
  if (label) ts = ts.filter(t => t.labels.includes(label));
  const search = q.get("q")?.toLowerCase();
  if (search) ts = ts.filter(t => `${t.title} ${t.description}`.toLowerCase().includes(search));
  const sort = q.get("sort") ?? "created";
  if (!["created", "-created", "priority"].includes(sort)) validation("sort");
  const rank = { high: 0, normal: 1, low: 2 };
  ts.sort((a,b) => sort === "priority" ?
    rank[a.priority as keyof typeof rank]-rank[b.priority as keyof typeof rank] || a.number-b.number :
    (sort === "-created" ? b.number-a.number : a.number-b.number));
  const limitText = q.get("limit") ?? "20";
  if (!/^\d+$/.test(limitText) || Number(limitText) < 1 || Number(limitText) > 100) validation("limit");
  const limit = Number(limitText), cursor = q.get("cursor");
  let start = 0;
  if (cursor) {
    try { const data = JSON.parse(atob(cursor)); if (data.s !== sort) validation("cursor");
      start = ts.findIndex(t => t.key === data.k)+1; if (!start) validation("cursor");
    } catch { validation("cursor"); }
  }
  const items = ts.slice(start, start+limit).map(clean), more = start+limit < ts.length;
  const nextCursor = more && items.length ?
    btoa(JSON.stringify({ s: sort, k: items[items.length-1]!.key })) : null;
  return { status: 200, body: { items, nextCursor } };
}
function patch(c: Ctx, t: Task): Result {
  member(c, t.projectId, true);
  const match = c.headers.get("If-Match");
  if (match && match !== `"${t.version}"`) fail(412, "precondition_failed", "Version changed");
  const b = c.body, allowed = ["title", "description", "priority", "labels", "assigneeId"];
  for (const k of Object.keys(b)) if (!allowed.includes(k)) validation(k);
  validate(c, t.projectId, { ...t, ...b }, "");
  const changes: any[] = [];
  for (const k of allowed.sort()) {
    if (!(k in b)) continue;
    let value = b[k];
    if (k === "title") value = value.trim();
    if (k === "labels") value = [...new Set(value.map((x: string) => x.trim().toLowerCase()))].sort();
    if (JSON.stringify((t as any)[k]) !== JSON.stringify(value)) {
      changes.push({ field: k, from: (t as any)[k], to: value });
      (t as any)[k] = value;
    }
  }
  if (changes.length) {
    t.version++; t.updatedAt = c.db.iso();
    for (const x of changes) t.history.push({ ...x, at: t.updatedAt, by: auth(c) });
  }
  return { status: 200, body: clean(t), headers: { ETag: `"${t.version}"` } };
}
function transition(c: Ctx, t: Task): Result {
  member(c, t.projectId, true);
  const to = c.body.to;
  if (!["open", "in_progress", "done"].includes(to)) validation("to");
  const valid = (t.status === "open" && to === "in_progress") ||
    (t.status === "in_progress" && ["open", "done"].includes(to)) ||
    (t.status === "done" && to === "open");
  if (!valid) fail(409, "invalid_transition", "Invalid transition");
  const match = c.headers.get("If-Match");
  if (match && match !== `"${t.version}"`) fail(412, "precondition_failed", "Version changed");
  if (to === "done" && [...t.deps].some(k => c.db.task(k)?.status !== "done"))
    fail(409, "blocked", "Dependencies are incomplete");
  const from = t.status; t.status = to; t.version++; t.updatedAt = c.db.iso();
  t.history.push({ at: t.updatedAt, by: auth(c), field: "status", from, to });
  return { status: 200, body: clean(t), headers: { ETag: `"${t.version}"` } };
}
function dependency(c: Ctx, t: Task): Result {
  const parts = c.path.split("/"), target = parts[4];
  if (c.method === "GET") return { status: 200, body: dependencyView(c, t) };
  if (c.method === "POST") {
    member(c, t.projectId, true);
    const key = c.body.dependsOn, other = c.db.task(key);
    if (typeof key !== "string" || key === t.key || !other || other.projectId !== t.projectId)
      validation("dependsOn");
    if (t.deps.has(key)) fail(409, "conflict", "Dependency exists");
    if (reaches(c, key, t.key)) fail(409, "dependency_cycle", "Dependency cycle");
    t.deps.add(key); return { status: 201, body: dependencyView(c, t) };
  }
  if (c.method === "DELETE" && target) {
    member(c, t.projectId, true);
    if (!t.deps.delete(target)) fail(404, "not_found", "Dependency not found");
    return { status: 204 };
  }
  fail(404, "not_found", "Not found");
}
function dependencyView(c: Ctx, t: Task) {
  const byNumber = (a: string, b: string) =>
    (c.db.task(a)?.number ?? 0)-(c.db.task(b)?.number ?? 0);
  return { key: t.key, dependsOn: [...t.deps].sort(byNumber),
    blocks: c.db.tasks.filter(x => x.deps.has(t.key)).map(x => x.key).sort(byNumber) };
}
function reaches(c: Ctx, from: string, to: string): boolean {
  if (from === to) return true;
  return [...(c.db.task(from)?.deps ?? [])].some(k => reaches(c, k, to));
}
function comments(c: Ctx, t: Task): Result {
  if (c.method === "GET") return { status: 200, body: t.comments };
  if (c.method !== "POST") return methodFail("GET, POST");
  const text = c.body.body;
  if (typeof text !== "string" || !text.trim() || text.length > 2000) validation("body");
  const comment = { id: t.comments.length+1, authorId: auth(c), body: text.trim(), createdAt: c.db.iso() };
  t.comments.push(comment); return { status: 201, body: comment };
}
