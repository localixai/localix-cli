import type { Store, Project, Task } from "./model";
import { iso, memberRole, taskByKey, projectForTask } from "./model";
import { auth, fail, ok, readBody, required } from "./http";
import { listTasks } from "./listing";

const member = (s: Store, p: Project, id: number, write = false) => {
  const role = memberRole(s, p, id);
  if (!role) fail(404, "not_found", "not found");
  if (write && role === "viewer") fail(403, "forbidden", "forbidden");
};
function validate(b: any, s: Store, p: Project, id: number, partial = false) {
  const allowed = ["title", "description", "priority", "labels", "assigneeId"];
  for (const k of Object.keys(b)) if (!allowed.includes(k)) fail(400, "validation", `invalid ${k}`, k);
  if (!partial || "title" in b) required(b.title, "title", (v) =>
    typeof v === "string" && v.trim().length > 0 && v.trim().length <= 200);
  if ("description" in b) required(b.description, "description", (v) => typeof v === "string");
  if ("priority" in b) required(b.priority, "priority", (v) => ["low", "normal", "high"].includes(v));
  if ("labels" in b) {
    required(b.labels, "labels", (v) => Array.isArray(v) && v.every((x) =>
      typeof x === "string" && x.trim().length > 0 && x.trim().length <= 30 && !/\s/.test(x.trim())));
  }
  if ("assigneeId" in b && b.assigneeId !== null) {
    required(b.assigneeId, "assigneeId", (v) => Number.isInteger(v) &&
      (v === p.ownerId || s.members.get(p.id)!.has(v)));
  }
}
function normalize(b: any) {
  const out = { ...b };
  if (out.title !== undefined) out.title = out.title.trim();
  if (out.labels !== undefined) out.labels = [...new Set(out.labels.map((x: string) => x.trim().toLowerCase()))].sort();
  return out;
}
function taskOut(t: Task) {
  const { deleted, deps, ...out } = t;
  return out;
}
function byKey(s: Store, key: string, id: number, write = false) {
  const t = taskByKey(s, key);
  if (!t) fail(404, "not_found", "not found");
  member(s, projectForTask(s, t), id, write);
  return t;
}
function etag(t: Task) { return { ETag: `"${t.version}"` }; }
function version(req: Request, t: Task) {
  const m = req.headers.get("If-Match");
  if (m && m !== `"${t.version}"`) fail(412, "precondition_failed", "task changed");
}
function history(s: Store, t: Task, by: number, field: string, from: any, to: any) {
  (s.history.get(t.id) ?? (s.history.set(t.id, []), s.history.get(t.id)!))
    .push({ at: iso(s), by, field, from, to });
}
function makeTask(s: Store, p: Project, b: any, by: number): Task {
  const v = normalize(b), number = p.next++;
  const t: Task = { id: ++s.tid, key: `${p.key}-${number}`, number, projectId: p.id,
    title: v.title, description: v.description ?? "", status: "open", priority: v.priority ?? "normal",
    labels: v.labels ?? [], assigneeId: v.assigneeId ?? null, createdBy: by,
    createdAt: iso(s), updatedAt: iso(s), version: 1, deleted: false, deps: [] };
  s.tasks.push(t); s.history.set(t.id, []); s.comments.set(t.id, []);
  return t;
}
export async function tasks(req: Request, url: URL, s: Store) {
  let m = url.pathname.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if (m) {
    const u = auth(req, s), p = s.projects.find((x) => x.id === +m![1]!);
    if (!p || !memberRole(s, p, u.id)) fail(404, "not_found", "not found");
    const role = memberRole(s, p, u.id);
    if (req.method === "GET" && !m[2]) return ok(listTasks(url, s, p, u.id));
    if (req.method === "POST") {
      if (role === "viewer") fail(403, "forbidden", "forbidden");
      const body = await readBody(req);
      if (m[2]) {
        if (!Array.isArray(body.tasks) || body.tasks.length < 1 || body.tasks.length > 50)
          fail(400, "validation", "invalid tasks", "tasks");
        body.tasks.forEach((x: any, i: number) => {
          try { validate(x, s, p, u.id); } catch (e) {
            if (e instanceof Error && "field" in e) (e as any).field = `tasks[${i}].${(e as any).field}`;
            throw e;
          }
        });
        return ok({ items: body.tasks.map((x: any) => taskOut(makeTask(s, p, x, u.id))) }, 201);
      }
      validate(body, s, p, u.id);
      const idem = req.headers.get("Idempotency-Key");
      if (idem) {
        const key = `${u.id}:${idem}`, hash = JSON.stringify(body), saved = s.idem.get(key);
        if (saved && saved.expires > s.now()) {
          if (saved.hash !== hash) fail(422, "idempotency_mismatch", "idempotency key mismatch");
          return ok(saved.body, 201, { ETag: '"1"', "Idempotent-Replayed": "true" });
        }
        const created = taskOut(makeTask(s, p, body, u.id));
        s.idem.set(key, { hash, body: created, expires: s.now() + 86400000 });
        return ok(created, 201, { ETag: '"1"' });
      }
      return ok(taskOut(makeTask(s, p, body, u.id)), 201, { ETag: '"1"' });
    }
  }
  m = url.pathname.match(
    /^\/tasks\/([A-Z]+-\d+)(?:\/(history|comments|transitions|dependencies)(?:\/([A-Z]+-\d+))?)?$/,
  );
  if (!m) return null;
  const u = auth(req, s), t = byKey(s, m[1]!, u.id);
  const sub = m[2];
  if (!sub && req.method === "GET") return ok(taskOut(t), 200, etag(t));
  if (!sub && req.method === "DELETE") {
    const role = memberRole(s, projectForTask(s, t), u.id);
    if (role === "viewer" || (role !== "owner" && t.createdBy !== u.id)) fail(403, "forbidden", "forbidden");
    t.deleted = true; s.tasks.forEach((x) => { x.deps = x.deps.filter((d) => d !== t.key); });
    return ok(null, 204);
  }
  if (!sub && req.method === "PATCH") {
    member(s, projectForTask(s, t), u.id, true);
    const b = await readBody(req); validate(b, s, projectForTask(s, t), u.id, true); version(req, t);
    const next = normalize(b); let changed = false;
    for (const field of Object.keys(next).sort()) {
      const from = (t as any)[field], to = next[field];
      if (JSON.stringify(from) !== JSON.stringify(to)) {
        history(s, t, u.id, field, from, to); (t as any)[field] = to; changed = true;
      }
    }
    if (changed) { t.version++; t.updatedAt = iso(s); }
    return ok(taskOut(t), 200, etag(t));
  }
  if (sub === "history" && req.method === "GET") return ok(s.history.get(t.id) ?? []);
  if (sub === "comments") {
    if (req.method === "GET") return ok(s.comments.get(t.id) ?? []);
    if (req.method === "POST") {
      const b = await readBody(req);
      const text = required(b.body, "body", (v) => typeof v === "string" && v.trim().length > 0 && v.length <= 2000);
      const c = { id: (s.comments.get(t.id)!.length + 1), authorId: u.id, body: text.trim(), createdAt: iso(s) };
      s.comments.get(t.id)!.push(c); return ok(c, 201);
    }
  }
  if (sub === "transitions" && req.method === "POST") {
    member(s, projectForTask(s, t), u.id, true);
    const b = await readBody(req); required(b.to, "to", (v) => ["open", "in_progress", "done"].includes(v));
    version(req, t);
    const valid = (t.status === "open" && b.to === "in_progress") ||
      (t.status === "in_progress" && ["open", "done"].includes(b.to)) || (t.status === "done" && b.to === "open");
    if (!valid) fail(409, "invalid_transition", "invalid transition");
    if (b.to === "done" && t.deps.some((k) => taskByKey(s, k)?.status !== "done"))
      fail(409, "blocked", "blocked");
    history(s, t, u.id, "status", t.status, b.to); t.status = b.to; t.version++; t.updatedAt = iso(s);
    return ok(taskOut(t), 200, etag(t));
  }
  if (sub === "dependencies") return dependencies(req, s, t, u.id, m[3]);
  return null;
}
function dependencies(req: Request, s: Store, t: Task, user: number, other?: string) {
  if (req.method === "GET") {
    const blocks = s.tasks.filter((x) => !x.deleted && x.deps.includes(t.key)).sort((a, b) => a.number - b.number);
    return ok({ key: t.key, dependsOn: t.deps.slice().sort((a, b) => +a.split("-")[1]! - +b.split("-")[1]!),
      blocks: blocks.map((x) => x.key) });
  }
  if (req.method === "DELETE" && other) {
    member(s, projectForTask(s, t), user, true);
    if (!t.deps.includes(other)) fail(404, "not_found", "not found");
    t.deps = t.deps.filter((x) => x !== other); return ok(null, 204);
  }
  if (req.method === "POST") {
    member(s, projectForTask(s, t), user, true);
    return readBody(req).then((b) => {
      const on = required(b.dependsOn, "dependsOn", (v) => typeof v === "string");
      const target = taskByKey(s, on);
      if (!target || target.projectId !== t.projectId || target.id === t.id)
        fail(400, "validation", "invalid dependency", "dependsOn");
      if (t.deps.includes(on)) fail(409, "conflict", "already depends");
      const reaches = (key: string, seen = new Set<string>()): boolean => {
        if (key === t.key) return true;
        if (seen.has(key)) return false;
        seen.add(key);
        return (taskByKey(s, key)?.deps ?? []).some((x) => reaches(x, seen));
      };
      if (reaches(on)) fail(409, "dependency_cycle", "dependency cycle");
      t.deps.push(on);
      return ok({ key: t.key, dependsOn: [on], blocks: [] }, 201);
    });
  }
  return null;
}
