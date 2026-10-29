import type { State, Task, History } from "./model";
import { auth, checkVersion, fail, projectOf, response, taskOf, time, validation, writable } from "./http";
import { body } from "./users";
import { list } from "./listing";

const priorities = ["high", "normal", "low"];
const statuses = ["open", "in_progress", "done"];
export async function taskRoutes(method: string, path: string[], req: Request, s: State,
  now: () => number, url: URL): Promise<Response> {
  const uid = auth(req, s);
  if (path[0] === "projects" && path[2] === "tasks") {
    const id = Number(path[1]);
    const p = /^\d+$/.test(path[1] || "") ? projectOf(s, id, uid) : undefined;
    if (!p) fail(404, "not_found", "Project not found");
    if (method === "GET" && path.length === 3) return list(s, p.id, uid, url);
    if ((method === "POST" && path.length === 3) || (method === "POST" && path[3] === "bulk")) {
      writable(s, p, uid);
      const b = await body(req);
      const bulk = path[3] === "bulk";
      const specs = bulk ? b.tasks : [b];
      if (!Array.isArray(specs) || specs.length < 1 || specs.length > 50) validation("tasks");
      const validated = specs.map((v: unknown, i: number) =>
        validateCreate(v, s, p.id, bulk ? `tasks[${i}]` : ""));
      if (bulk) {
        const made = validated.map((v: any) => create(s, p.id, uid, v, now));
        return tagged({ items: made }, 201, made[0]?.version);
      }
      const idem = req.headers.get("Idempotency-Key");
      if (idem) {
        const key = `${uid}:${idem}`, hash = JSON.stringify(b);
        const old = s.idem.get(key);
        if (old && now() - old.at < 86400000) {
          if (old.hash !== hash) fail(422, "idempotency_mismatch", "Idempotency key has another body");
          return tagged(old.body, 201, (old.body as Task).version, { "Idempotent-Replayed": "true" });
        }
        const made = create(s, p.id, uid, validated[0], now);
        s.idem.set(key, { hash, body: made, at: now() });
        return tagged(made, 201, made.version);
      }
      const made = create(s, p.id, uid, validated[0], now);
      return tagged(made, 201, made.version);
    }
  }
  if (path[0] !== "tasks" || !path[1]) return response({ error: { code: "not_found", message: "Not found" } }, 404);
  const t = taskOf(s, path[1], uid), p = s.projects.find((x) => x.id === t.projectId)!;
  if (path.length === 2 && method === "GET") return tagged(t, 200, t.version);
  if (path.length === 2 && method === "PATCH") {
    writable(s, p, uid);
    checkVersion(req, t);
    const b = await body(req);
    const fields = Object.keys(b).sort();
    const allowed = ["assigneeId", "description", "labels", "priority", "title"];
    for (const f of fields) if (!allowed.includes(f)) validation(f);
    const next = validateCreate({ ...t, ...b }, s, p.id, "");
    let changed = false;
    for (const f of fields) {
      const value = (next as any)[f];
      if (JSON.stringify((t as any)[f]) !== JSON.stringify(value)) {
        pushHistory(s, t, uid, f, (t as any)[f], value, now);
        (t as any)[f] = value;
        changed = true;
      }
    }
    if (changed) bump(t, now);
    return tagged(t, 200, t.version);
  }
  if (path[2] === "transitions" && method === "POST") {
    writable(s, p, uid); checkVersion(req, t);
    const b = await body(req);
    if (!statuses.includes(b.to)) validation("to");
    const allowed: Record<string, string[]> = { open: ["in_progress"], in_progress: ["open", "done"], done: ["open"] };
    if (!(allowed[t.status] || []).includes(b.to)) fail(409, "invalid_transition", "Invalid status transition");
    if (b.to === "done" && [...(s.deps.get(t.id) || [])].some((d) =>
      s.tasks.find((x) => x.id === d)?.status !== "done")) {
      fail(409, "blocked", "A dependency is not done");
    }
    pushHistory(s, t, uid, "status", t.status, b.to, now); t.status = b.to; bump(t, now);
    return tagged(t, 200, t.version);
  }
  if (path[2] === "history" && method === "GET") return response(s.history.get(t.id) || []);
  if (path[2] === "comments") return comments(method, path, req, s, uid, t.id, now);
  if (path[2] === "dependencies") return dependencies(method, path, req, s, uid, t);
  if (path.length === 2 && method === "DELETE") {
    writable(s, p, uid);
    if (t.createdBy !== uid && p.ownerId !== uid) fail(403, "forbidden", "You may only delete your own tasks");
    s.tasks = s.tasks.filter((x) => x.id !== t.id);
    s.deps.delete(t.id);
    for (const deps of s.deps.values()) deps.delete(t.id);
    s.comments.delete(t.id); s.history.delete(t.id);
    return response(null, 204);
  }
  return response(null, 405, { Allow: "GET, PATCH, DELETE" });
}
function validateCreate(v: unknown, s: State, pid: number, prefix: string): any {
  const b = v && typeof v === "object" && !Array.isArray(v) ? v as any : {};
  const field = (x: string) => validation(prefix ? `${prefix}.${x}` : x);
  if (typeof b.title !== "string" || !b.title.trim() || b.title.length > 200) field("title");
  if (b.description !== undefined && typeof b.description !== "string") field("description");
  if (b.priority !== undefined && !priorities.includes(b.priority)) field("priority");
  if (b.labels !== undefined && (!Array.isArray(b.labels) || b.labels.some((x: unknown) =>
    typeof x !== "string" || !x.trim() || x.trim().length > 30 || /\s/.test(x.trim())))) field("labels");
  if (b.assigneeId !== undefined && b.assigneeId !== null &&
    !s.members.get(pid)?.some((m) => m.userId === b.assigneeId)) field("assigneeId");
  return { title: b.title.trim(), description: b.description ?? "",
    priority: b.priority ?? "normal",
    labels: [...new Set((b.labels ?? []).map((x: string) =>
      x.trim().toLowerCase()))].sort(),
    assigneeId: b.assigneeId ?? null };
}
function create(s: State, pid: number, uid: number, b: any, now: () => number): Task {
  const p = s.projects.find((x) => x.id === pid)!;
  const n = p.next++;
  const at = time(now);
  const t: Task = { id: ++s.ids.task, projectId: pid, key: `${p.key}-${n}`, number: n, ...b,
    status: "open", createdBy: uid, createdAt: at, updatedAt: at, version: 1 };
  s.tasks.push(t); s.history.set(t.id, []); s.deps.set(t.id, new Set()); s.comments.set(t.id, []);
  return t;
}
function bump(t: Task, now: () => number) { t.version++; t.updatedAt = time(now); }
function pushHistory(s: State, t: Task, by: number, field: string, from: unknown, to: unknown, now: () => number) {
  s.history.get(t.id)!.push({ at: time(now), by, field, from, to });
}
function tagged(body: unknown, status: number, version?: number, extra: Record<string, string> | Headers = {}) {
  const h = new Headers(extra); if (version) h.set("ETag", `"${version}"`);
  return response(body, status, h);
}
async function comments(method: string, path: string[], req: Request, s: State,
  uid: number, tid: number, now: () => number): Promise<Response> {
  const list = s.comments.get(tid)!;
  if (method === "GET") return response(list);
  if (method !== "POST") return response(null, 405, { Allow: "GET, POST" });
  const b = await body(req);
  if (typeof b.body !== "string" || !b.body.trim() || b.body.length > 2000) validation("body");
  const item = { id: ++s.ids.comment, authorId: uid, body: b.body, createdAt: time(now) };
  list.push(item);
  return response(item, 201);
}
async function dependencies(method: string, path: string[], req: Request, s: State,
  uid: number, t: Task): Promise<Response> {
  const deps = s.deps.get(t.id)!;
  if (method === "GET") return response(depView(s, t));
  if (method === "POST" && path.length === 3) {
    const p = s.projects.find((x) => x.id === t.projectId)!;
    writable(s, p, uid);
    const b = await body(req);
    const other = s.tasks.find((x) => x.key === b.dependsOn && x.projectId === t.projectId);
    if (!other || other.id === t.id) validation("dependsOn");
    const dependency = other!;
    if (deps.has(dependency.id)) fail(409, "conflict", "Dependency already exists");
    if (reaches(s, dependency.id, t.id)) fail(409, "dependency_cycle", "Dependency would create a cycle");
    deps.add(dependency.id);
    return response(depView(s, t), 201);
  }
  if (method === "DELETE" && path.length === 4) {
    const p = s.projects.find((x) => x.id === t.projectId)!;
    writable(s, p, uid);
    const other = s.tasks.find((x) => x.key === path[3]);
    if (!other || !deps.delete(other.id)) fail(404, "not_found", "Dependency not found");
    return response(null, 204);
  }
  return response(null, 405, { Allow: "GET, POST, DELETE" });
}
function depView(s: State, t: Task) {
  const keys = (ids: number[]) => ids.map((id) => s.tasks.find((x) => x.id === id)!)
    .filter(Boolean).sort((a, b) => a.number - b.number).map((x) => x.key);
  return { key: t.key, dependsOn: keys([...(s.deps.get(t.id) || [])]),
    blocks: keys([...s.deps].filter(([, ds]) => ds.has(t.id)).map(([id]) => id)) };
}
function reaches(s: State, start: number, target: number): boolean {
  const seen = new Set<number>();
  const visit = (id: number): boolean => {
    if (id === target) return true;
    if (seen.has(id)) return false;
    seen.add(id);
    return [...(s.deps.get(id) || [])].some(visit);
  };
  return visit(start);
}
