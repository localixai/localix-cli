import { auth, body, date, etag, fail, json, project, role, task, taskId, validate, writeTask, type Ctx } from "./util";
import { taskKey } from "./model";
function labels(value: any): string[] {
  validate(Array.isArray(value), "labels");
  const out: string[] = [];
  for (const item of value) {
    const label = typeof item === "string" ? item.trim() : "";
    validate(label.length > 0 && label.length <= 30 && !/\s/.test(label), "labels");
    if (!out.includes(item.trim().toLowerCase())) out.push(item.trim().toLowerCase());
  }
  return out.sort();
}
function validateTask(b: any, projectId: number, c: Ctx, prefix = "") {
  const field = (name: string) => prefix ? `${prefix}.${name}` : name;
  validate(typeof b.title === "string" && b.title.trim().length > 0 && b.title.trim().length <= 200, field("title"));
  validate(b.description === undefined || typeof b.description === "string", field("description"));
  validate(b.priority === undefined || ["low", "normal", "high"].includes(b.priority), field("priority"));
  const ls = b.labels === undefined ? [] : labels(b.labels);
  if (b.assigneeId !== undefined && b.assigneeId !== null) validate(
    c.state.members.get(projectId)?.has(b.assigneeId), field("assigneeId"));
  return { title: b.title.trim(), description: b.description ?? "", priority: b.priority ?? "normal",
    labels: ls, assigneeId: b.assigneeId ?? null };
}
function create(c: Ctx, pid: number, b: any, by: number) {
  const p = c.state.projects.find((x) => x.id === pid)!;
  const values = validateTask(b, pid, c);
  const number = (c.state.taskNumbers.get(pid) || 0) + 1;
  c.state.taskNumbers.set(pid, number);
  const at = date(c);
  const id = c.state.counters.task = (c.state.counters.task || 0) + 1;
  const t = { id, number, key: `${p.key}-${number}`, projectId: pid, ...values, status: "open",
    createdBy: by, createdAt: at, updatedAt: at, version: 1 };
  c.state.tasks.push(t); return t;
}
export async function tasks(c: Ctx, method: string, path: string): Promise<Response | undefined> {
  const s = c.state;
  const bulk = /^\/projects\/(\d+)\/tasks\/bulk$/.exec(path);
  const list = /^\/projects\/(\d+)\/tasks$/.exec(path);
  if (bulk && method === "POST") {
    const pid = Number(bulk[1]); project(c, pid);
    if (!["owner", "member"].includes(role(c, pid)!)) fail(403, "forbidden", "Member required");
    const b = await body(c.request);
    validate(Array.isArray(b.tasks) && b.tasks.length >= 1 && b.tasks.length <= 50, "tasks");
    b.tasks.forEach((x: any, i: number) => validateTask(x, pid, c, `tasks[${i}]`));
    const user = auth(c).user!; return json({ items: b.tasks.map((x: any) => create(c, pid, x, user.id)) }, 201);
  }
  if (list) {
    const pid = Number(list[1]); project(c, pid);
    if (method === "POST") {
      if (["owner", "member"].indexOf(role(c, pid)!) < 0) fail(403, "forbidden", "Member required");
      const b = await body(c.request); const user = auth(c).user!;
      const key = c.request.headers.get("Idempotency-Key"); const ik = key && `${user.id}:${key}`;
      const old = ik ? s.idem.get(ik) : undefined;
      const fingerprint = JSON.stringify(b);
      if (old && c.now() - old.at < 86400000) {
        if (old.fingerprint !== fingerprint) fail(422, "idempotency_mismatch", "Key used with another body");
        return json(old.result, 201, { ETag: '"1"', "Idempotent-Replayed": "true" });
      }
      const t = create(c, pid, b, user.id);
      if (ik) s.idem.set(ik, { fingerprint, result: t, at: c.now() });
      return json(t, 201, etag(t));
    }
    if (method === "GET") return listTasks(c, pid);
  }
  return await single(c, method, path);
}
function listTasks(c: Ctx, pid: number) {
  const url = new URL(c.request.url); const q = url.searchParams;
  let items = c.state.tasks.filter((t) => t.projectId === pid);
  const status = q.get("status");
  if (status) {
    validate(["open", "in_progress", "done"].includes(status), "status");
    items = items.filter((t) => t.status === status);
  }
  const assignee = q.get("assignee");
  if (assignee) {
    const me = auth(c).user!.id;
    validate(["me", "none"].includes(assignee) || /^\d+$/.test(assignee), "assignee");
    items = items.filter((t) => assignee === "me" ? t.assigneeId === me :
      assignee === "none" ? t.assigneeId === null : t.assigneeId === Number(assignee));
  }
  const label = q.get("label"); if (label) items = items.filter((t) => t.labels.includes(label.toLowerCase()));
  const search = q.get("q")?.toLowerCase();
  if (search) items = items.filter((t) => `${t.title} ${t.description}`.toLowerCase().includes(search));
  const sort = q.get("sort") || "created";
  validate(["created", "-created", "priority"].includes(sort), "sort");
  items.sort((a, b) => sort === "-created" ? b.number - a.number : sort === "priority" ?
    ["high", "normal", "low"].indexOf(a.priority) -
      ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number :
    a.number - b.number);
  let limit = 20;
  if (q.has("limit")) { const raw = q.get("limit")!; limit = Number(raw);
    validate(/^\d+$/.test(raw) && limit >= 1 && limit <= 100, "limit"); }
  const cursor = q.get("cursor"); let offset = 0;
  if (cursor) { try { offset = Number(atob(cursor)); validate(Number.isInteger(offset) && offset >= 0, "cursor"); }
    catch { fail(400, "validation", "Invalid cursor", "cursor"); } }
  const page = items.slice(offset, offset + limit);
  const nextCursor = offset + page.length < items.length ? btoa(String(offset + page.length)) : null;
  return json({ items: page, nextCursor });
}
async function single(c: Ctx, method: string, path: string): Promise<Response | undefined> {
  const m = /^\/tasks\/([^/]+)(?:\/(.+))?$/.exec(path); if (!m) return;
  const key = m[1]!; const t = task(c, key); const sub = m[2]; const s = c.state;
  if (!sub && method === "GET") return json(t, 200, etag(t));
  const current = auth(c).user!; const memberRole = role(c, t.projectId);
  if (sub === "history" && method === "GET") return json(s.histories.get(taskId(t)) || []);
  if (sub?.startsWith("dependencies")) return dependency(c, method, t, m[1]!, path);
  if (sub === "comments") return comments(c, method, t);
  if (method === "DELETE" && !sub) {
    if (memberRole !== "owner" && t.createdBy !== current.id) fail(403, "forbidden", "Cannot delete task");
    s.tasks = s.tasks.filter((x) => x !== t); s.dependencies.delete(key);
    for (const set of s.dependencies.values()) set.delete(key);
    return new Response(null, { status: 204 });
  }
  if (method === "PATCH" && !sub || method === "POST" && sub === "transitions") {
    if (memberRole === "viewer") fail(403, "forbidden", "Member required");
    const match = c.request.headers.get("If-Match");
    if (match && match !== `"${t.version}"`) fail(412, "precondition_failed", "Task changed");
    const b = await body(c.request);
    if (sub === "transitions") {
      validate(["open", "in_progress", "done"].includes(b.to), "to");
      const valid = t.status === "open" && b.to === "in_progress" ||
        t.status === "in_progress" && ["open", "done"].includes(b.to) ||
        t.status === "done" && b.to === "open";
      if (!valid) fail(409, "invalid_transition", "Invalid transition");
      if (b.to === "done" && [...(s.dependencies.get(key) || [])].some((k) => {
      const dependency = s.tasks.find((x) => taskKey(s, x) === k);
      return dependency && dependency.status !== "done";
      })) fail(409, "blocked", "Dependencies are not done");
      const { writeTask } = await import("./util");
      writeTask(c, t, { status: b.to }, current.id); return json(t, 200, etag(t));
    }
    for (const k of Object.keys(b)) {
      validate(["title", "description", "priority", "labels", "assigneeId"].includes(k), k);
    }
    const values: Record<string, any> = validateTask({ ...t, ...b }, t.projectId, c);
    const fields: Record<string, any> = {};
    for (const k of Object.keys(b)) fields[k] = values[k];
    const { writeTask } = await import("./util"); writeTask(c, t, fields, current.id);
    return json(t, 200, etag(t));
  }
}
async function dependency(c: Ctx, method: string, t: any, key: string, path: string) {
  const s = c.state; const map = s.dependencies; let own = map.get(key);
  if (!own) { own = new Set(); map.set(key, own); }
  if (method === "GET") {
    const blocks = [...map].filter(([, vals]) => vals.has(key)).map(([k]) => k)
      .sort((a, b) => Number(a.split("-").pop()) - Number(b.split("-").pop()));
    const order = (a: string, b: string) =>
      Number(a.split("-").pop()) - Number(b.split("-").pop());
    return json({ key, dependsOn: [...own].sort(order), blocks });
  }
  const del = /^\/tasks\/[^/]+\/dependencies\/([^/]+)$/.exec(path);
  if (method === "DELETE" && del) {
    if (!own.delete(del[1]!)) fail(404, "not_found", "Dependency not found");
    return new Response(null, { status: 204 });
  }
  if (method === "POST") {
    if (role(c, t.projectId) === "viewer") fail(403, "forbidden", "Member required");
    const b = await body(c.request); const other = typeof b.dependsOn === "string" ? b.dependsOn : "";
    const dep = s.tasks.find((x) => taskKey(s, x) === other);
    validate(dep && dep.projectId === t.projectId && other !== key, "dependsOn");
    if (own.has(other)) fail(409, "conflict", "Dependency exists");
    const reaches = (from: string, target: string, seen = new Set<string>()): boolean => {
      if (from === target) return true; if (seen.has(from)) return false; seen.add(from);
      return [...(map.get(from) || [])].some((next) => reaches(next, target, seen));
    };
    if (reaches(other, key)) fail(409, "dependency_cycle", "Dependency creates a cycle");
    own.add(other); return json({ key, dependsOn: [...own], blocks: [] }, 201);
  }
}
async function comments(c: Ctx, method: string, t: any) {
  const s = c.state; const id = taskId(t); const list = s.comments.get(id) || [];
  if (method === "GET") return json(list);
  if (method === "POST") {
    const b = await body(c.request);
    validate(typeof b.body === "string" && b.body.trim().length > 0 && b.body.length <= 2000, "body");
    const user = auth(c).user!;
    const item = { id: s.counters.comment = (s.counters.comment || 0) + 1,
      authorId: user.id, body: b.body,
      createdAt: date(c) }; list.push(item); s.comments.set(id, list); return json(item, 201);
  }
}
