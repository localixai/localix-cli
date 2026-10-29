import { State, Task, Project, membership, visibleTask, iso, bad, error, notFound } from "./model";

function normalize(data: Record<string, any>) {
  const title = typeof data.title === "string" ? data.title.trim() : "";
  if (!title || title.length > 200) return "title";
  if (data.description !== undefined && typeof data.description !== "string") return "description";
  if (data.priority !== undefined && !["low", "normal", "high"].includes(data.priority)) return "priority";
  if (data.labels !== undefined && (!Array.isArray(data.labels) ||
    data.labels.some((x: any) => typeof x !== "string" || !x.trim() || x.trim().length > 30 ||
      /\s/.test(x.trim())))) return "labels";
  if (data.assigneeId !== undefined && data.assigneeId !== null && !Number.isInteger(data.assigneeId))
    return "assigneeId";
  return null;
}
function make(s: State, p: Project, user: number, data: Record<string, any>): Task {
  const number = (s.taskNumbers.get(p.id) ?? 0) + 1;
  s.taskNumbers.set(p.id, number);
  const time = iso(s);
  const t: Task = { id: ++s.ids.task, key: `${p.key}-${number}`, number, projectId: p.id,
    title: data.title.trim(), description: data.description ?? "", status: "open",
    priority: data.priority ?? "normal",
    labels: [...new Set<string>((data.labels ?? []).map((x: string) => x.trim().toLowerCase()))].sort(),
    assigneeId: data.assigneeId ?? null, createdBy: user, createdAt: time, updatedAt: time, version: 1 };
  s.tasks.push(t); s.history.set(t.id, []); s.comments.set(t.id, []); s.deps.set(t.id, new Set());
  return t;
}
export function taskRoute(s: State, method: string, path: string, user: number,
  data: Record<string, any> | null, url: URL, headers: Headers): Response | undefined {
  const pm = path.match(/^\/projects\/(\d+)\/tasks(\/bulk)?$/);
  if (pm) {
    const p = s.projects.find(x => x.id === Number(pm[1]));
    if (!p || !membership(s, p, user)) return notFound();
    const role = membership(s, p, user);
    if (method === "POST") {
      if (role === "viewer") return error("forbidden", "Forbidden", 403);
      const inputs = pm[2] ? data?.tasks : [data];
      if (!Array.isArray(inputs) || !inputs.length || inputs.length > (pm[2] ? 50 : 1))
        return bad(pm[2] ? "tasks" : "body");
      for (let i = 0; i < inputs.length; i++) {
        const issue = normalize(inputs[i] ?? {});
        if (issue) return bad(pm[2] ? `tasks[${i}].${issue}` : issue);
        const id = inputs[i].assigneeId;
        if (id != null && !membership(s, p, id)) return bad(pm[2] ? `tasks[${i}].assigneeId` : "assigneeId");
      }
      const idemKey = headers.get("Idempotency-Key");
      const idemId = idemKey ? `${user}:${idemKey}` : "";
      const saved = idemId ? s.idem.get(idemId) : undefined;
      if (saved && s.now() - saved.at < 86400000) {
        if (saved.body !== JSON.stringify(data))
          return error("idempotency_mismatch", "Key reused with another body", 422);
        return Response.json(saved.result, { status: 201, headers: { "Idempotent-Replayed": "true",
          ETag: `"${saved.result.version}"` } });
      }
      const made = inputs.map(x => make(s, p, user, x));
      const result = pm[2] ? { items: made } : made[0]!;
      if (idemId && !pm[2]) s.idem.set(idemId, { at: s.now(), body: JSON.stringify(data), result: made[0]! });
      const created = made[0]!;
      return Response.json(result, { status: 201,
        headers: !pm[2] ? { ETag: `"${created.version}"` } : {} });
    }
    if (method === "GET" && !pm[2]) return listTasks(s, p, user, url);
  }
  const tm = path.match(/^\/tasks\/([A-Z]+-\d+)(?:\/(comments|history|transitions|dependencies))?$/);
  if (!tm) return;
  const t = visibleTask(s, tm[1]!, user);
  if (!t) return notFound();
  const p = s.projects.find(x => x.id === t.projectId)!;
  const role = membership(s, p, user)!;
  const sub = tm[2];
  if (!sub && method === "GET") return Response.json(t, { headers: { ETag: `"${t.version}"` } });
  if (!sub && method === "DELETE") {
    if (role === "viewer" || (role !== "owner" && t.createdBy !== user))
      return error("forbidden", "Forbidden", 403);
    s.tasks = s.tasks.filter(x => x.id !== t.id);
    s.deps.delete(t.id);
    for (const set of s.deps.values()) set.delete(t.id);
    return new Response(null, { status: 204 });
  }
  if (!sub && method === "PATCH") {
    if (role === "viewer") return error("forbidden", "Forbidden", 403);
    if (headers.has("If-Match") && headers.get("If-Match") !== `"${t.version}"`)
      return error("precondition_failed", "Task changed", 412);
    const allowed = ["title", "description", "priority", "labels", "assigneeId"];
    const unknown = Object.keys(data ?? {}).find(k => !allowed.includes(k));
    if (unknown) return bad(unknown);
    const patch = data ?? {};
    if (patch.title !== undefined && (typeof patch.title !== "string" ||
      !patch.title.trim() || patch.title.trim().length > 200)) return bad("title");
    if (patch.description !== undefined && typeof patch.description !== "string") return bad("description");
    if (patch.priority !== undefined && !["low", "normal", "high"].includes(patch.priority))
      return bad("priority");
    if (patch.labels !== undefined && normalize({ ...t, ...patch }) === "labels") return bad("labels");
    const before: any = { ...t };
    for (const k of Object.keys(data!)) (t as any)[k] = k === "title" ? data![k].trim() :
      k === "labels" ? [...new Set(data![k].map((x: string) => x.trim().toLowerCase()))].sort() : data![k];
    record(s, t, user, before);
    return Response.json(t, { headers: { ETag: `"${t.version}"` } });
  }
  if (sub === "transitions" && method === "POST") return transition(s, t, user, role, data, headers);
  if (sub === "history" && method === "GET") return Response.json(s.history.get(t.id) ?? []);
  if (sub === "comments") {
    if (method === "GET") return Response.json(s.comments.get(t.id) ?? []);
    if (method === "POST") {
      const text = typeof data?.body === "string" ? data.body.trim() : "";
      if (!text || text.length > 2000) return bad("body");
      const c = { id: ++s.ids.comment, authorId: user, body: text, createdAt: iso(s) };
      s.comments.get(t.id)!.push(c); return Response.json(c, { status: 201 });
    }
  }
}
function record(s: State, t: Task, user: number, old: any) {
  const fields = ["assigneeId", "description", "labels", "priority", "status", "title"];
  const changed = fields.filter(k => JSON.stringify(old[k]) !== JSON.stringify((t as any)[k]));
  if (!changed.length) return;
  t.version++; t.updatedAt = iso(s);
  changed.forEach(field => s.history.get(t.id)!.push({
    at: t.updatedAt, by: user, field, from: old[field], to: (t as any)[field],
  }));
}
function transition(s: State, t: Task, user: number, role: string, data: any, headers: Headers) {
  if (role === "viewer") return error("forbidden", "Forbidden", 403);
  if (!["open", "in_progress", "done"].includes(data?.to)) return bad("to");
  if (headers.has("If-Match") && headers.get("If-Match") !== `"${t.version}"`)
    return error("precondition_failed", "Task changed", 412);
  const allowed = t.status === "open" ? ["in_progress"] : t.status === "in_progress" ?
    ["open", "done"] : ["open"];
  if (!allowed.includes(data.to)) return error("invalid_transition", "Invalid transition", 409);
  if (data.to === "done" && [...(s.deps.get(t.id) ?? [])].some(id =>
    s.tasks.find(x => x.id === id)?.status !== "done")) return error("blocked", "Dependencies remain", 409);
  const old = { ...t }; t.status = data.to; record(s, t, user, old);
  return Response.json(t, { headers: { ETag: `"${t.version}"` } });
}
function listTasks(s: State, p: Project, user: number, url: URL) {
  let tasks = s.tasks.filter(t => t.projectId === p.id);
  const q = url.searchParams.get("q")?.toLowerCase();
  for (const [key, value] of url.searchParams) {
    if (key === "status") {
      if (!["open", "in_progress", "done"].includes(value)) return bad(key);
      tasks = tasks.filter(t => t.status === value);
    }
    if (key === "label") tasks = tasks.filter(t => t.labels.includes(value.toLowerCase()));
    if (key === "q") tasks = tasks.filter(t => `${t.title} ${t.description}`.toLowerCase().includes(q!));
    if (key === "assignee") {
      if (value === "none") tasks = tasks.filter(t => t.assigneeId === null);
      else if (value === "me") tasks = tasks.filter(t => t.assigneeId === user);
      else if (/^\d+$/.test(value)) tasks = tasks.filter(t => t.assigneeId === Number(value));
      else return bad(key);
    }
  }
  const sort = url.searchParams.get("sort") ?? "created";
  if (!["created", "-created", "priority"].includes(sort)) return bad("sort");
  tasks.sort((a, b) => sort === "-created" ? b.number - a.number :
    sort === "priority" ? ["high", "normal", "low"].indexOf(a.priority) -
      ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number : a.number - b.number);
  const limitText = url.searchParams.get("limit") ?? "20", limit = Number(limitText);
  if (!/^\d+$/.test(limitText) || limit < 1 || limit > 100) return bad("limit");
  let offset = 0;
  const cursor = url.searchParams.get("cursor");
  if (cursor) {
    try { offset = Number(atob(cursor)); if (!Number.isInteger(offset)) throw 0; }
    catch { return bad("cursor"); }
  }
  const items = tasks.slice(offset, offset + limit);
  return Response.json({ items, nextCursor: offset + limit < tasks.length ? btoa(String(offset + limit)) : null });
}
