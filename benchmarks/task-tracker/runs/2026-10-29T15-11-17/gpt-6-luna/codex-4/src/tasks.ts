import type { State, Task } from "./model";
import { taskView } from "./model";
import { extras, listTasks } from "./task_extra";
import { check, fail, labels, memberRole, projectForTask, taskFor } from "./util";

const validate = (s: State, p: any, b: Record<string, any>, create: boolean) => {
  const allowed = create
    ? ["title", "description", "priority", "labels", "assigneeId"]
    : ["title", "description", "priority", "labels", "assigneeId"];
  for (const k of Object.keys(b)) if (!allowed.includes(k)) check(false, k);
  if (create || "title" in b)
    check(typeof b.title === "string" && b.title.trim().length > 0 && b.title.trim().length <= 200, "title");
  if ("description" in b) check(typeof b.description === "string", "description");
  if ("priority" in b) check(["low", "normal", "high"].includes(b.priority), "priority");
  if ("labels" in b) b.labels = labels(b.labels);
  if ("assigneeId" in b && b.assigneeId !== null) {
    check(Number.isInteger(b.assigneeId) && !!memberRole(s, p, b.assigneeId), "assigneeId");
  }
};
const accessible = (s: State, key: string, uid: number) => {
  const t = taskFor(s, key);
  const p = t && projectForTask(s, t);
  if (!t || !p || !memberRole(s, p, uid)) fail(404, "not_found", "task not found");
  return { t, p };
};
const makeTask = (s: State, p: any, b: Record<string, any>, uid: number, now: number): Task => {
  validate(s, p, b, true);
  const d = new Date(now).toISOString();
  const t: Task = {
    id: s.tasks.length + 1,
    number: p.next++,
    projectId: p.id,
    title: b.title.trim(),
    description: b.description ?? "",
    status: "open",
    priority: b.priority ?? "normal",
    labels: b.labels ?? [],
    assigneeId: b.assigneeId ?? null,
    createdBy: uid,
    createdAt: d,
    updatedAt: d,
    version: 1,
  };
  s.tasks.push(t);
  s.history.set(t.id, []);
  s.comments.set(t.id, []);
  s.deps.set(t.id, new Set());
  return t;
};
export function tasks(
  s: State,
  method: string,
  path: string,
  url: URL,
  b: Record<string, any>,
  uid: number,
  now: number,
  headers: Headers,
) {
  const bulk = /^\/projects\/(\d+)\/tasks\/bulk$/.exec(path);
  const list = /^\/projects\/(\d+)\/tasks$/.exec(path);
  if (list || bulk) {
    const p = s.projects.find((x) => x.id === Number((list || bulk)![1]));
    if (!p || !memberRole(s, p, uid)) fail(404, "not_found", "project not found");
    const role = memberRole(s, p, uid);
    if (method === "POST") {
      if (role === "viewer") fail(403, "forbidden", "viewers cannot create tasks");
      if (bulk) {
        check(Array.isArray(b.tasks) && b.tasks.length >= 1 && b.tasks.length <= 50, "tasks");
        for (let i = 0; i < b.tasks.length; i++) {
          try {
            validate(s, p, b.tasks[i], true);
          } catch (e) {
            if (e && typeof e === "object" && "field" in e) (e as any).field = `tasks[${i}].${(e as any).field}`;
            throw e;
          }
        }
        return {
          status: 201,
          body: {
            items: b.tasks.map((x: any) => taskView(makeTask(s, p, x, uid, now), p)),
          },
        };
      }
      const ik = headers.get("Idempotency-Key");
      const old = ik && s.idems.get(`${uid}:${ik}`);
      if (old && now - old.at < 86400000) {
        if (old.body !== JSON.stringify(b)) fail(422, "idempotency_mismatch", "key used with another body");
        return { status: 201, body: old.result, replay: true };
      }
      const t = makeTask(s, p, b, uid, now),
        result = taskView(t, p);
      if (ik)
        s.idems.set(`${uid}:${ik}`, {
          at: now,
          body: JSON.stringify(b),
          result,
        });
      return { status: 201, body: result, etag: t.version };
    }
    if (method === "GET") return listTasks(s, p, url, uid);
  }
  const m = /^\/tasks\/([^/]+)(?:\/(.*))?$/.exec(path);
  if (!m) return;
  const { t, p } = accessible(s, decodeURIComponent(m[1]!), uid);
  const sub = m[2] || "";
  if (sub === "" && method === "GET") return { status: 200, body: taskView(t, p), etag: t.version };
  if (sub === "" && method === "PATCH") {
    if (memberRole(s, p, uid) === "viewer") fail(403, "forbidden", "viewers cannot edit tasks");
    precondition(t, headers);
    validate(s, p, b, false);
    const changes: any[] = [];
    for (const k of Object.keys(b).sort()) {
      const value = k === "title" ? b[k].trim() : b[k];
      if (JSON.stringify((t as any)[k]) !== JSON.stringify(value)) changes.push([k, (t as any)[k], value]);
    }
    if (changes.length) {
      for (const [k, from, to] of changes) {
        (t as any)[k] = to;
        s.history.get(t.id)!.push({
          at: new Date(now).toISOString(),
          by: uid,
          field: k,
          from,
          to,
        });
      }
      t.version++;
      t.updatedAt = new Date(now).toISOString();
    }
    return { status: 200, body: taskView(t, p), etag: t.version };
  }
  if (sub === "" && method === "DELETE") {
    if (memberRole(s, p, uid) === "viewer" || (uid !== p.ownerId && t.createdBy !== uid))
      fail(403, "forbidden", "cannot delete this task");
    s.tasks = s.tasks.filter((x) => x !== t);
    s.deps.delete(t.id);
    for (const set of s.deps.values()) set.delete(t.id);
    return { status: 204, body: null };
  }
  return extras(s, method, sub, t, p, b, uid, now, headers);
}
function precondition(t: Task, h: Headers) {
  const value = h.get("If-Match");
  if (value && value !== `"${t.version}"`) fail(412, "precondition_failed", "task version changed");
}
