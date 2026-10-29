import { comments, dependencies, transition } from "./task-extra";
import { listing, stats } from "./listing";
import { State, Task, Project, stamp, taskView } from "./state";
import {
  answer,
  auth,
  bodyObj,
  fail,
  project,
  task,
  writeRole,
  validateTask,
  labels,
  etag,
  precondition,
} from "./http";
export function tasksRoute(
  s: State,
  r: Request,
  m: string,
  url: URL,
  now: () => number,
): Response | undefined {
  const a = url.pathname.split("/").filter(Boolean),
    b = (r as any).__body || {};
  if (a[0] === "projects" && a[2] === "tasks") {
    const u = auth(s, r),
      p = project(s, a[1]!, u.id);
    if (a.length === 3 && m === "GET") return listing(s, url, p, u.id);
    writeRole(s, p, u.id);
    if (a.length === 3 && m === "POST") return create(s, p, u.id, b, r, now);
    if (a.length === 4 && a[3] === "bulk" && m === "POST") {
      if (!Array.isArray(b.tasks) || b.tasks.length < 1 || b.tasks.length > 50)
        fail(400, "validation", "Invalid tasks", "tasks");
      for (let i = 0; i < b.tasks.length; i++) {
        try {
          validateTask(b.tasks[i]);
          const aid = b.tasks[i].assigneeId;
          if (aid != null && !s.members.get(p.id)?.has(aid))
            fail(400, "validation", "Invalid assignee", "assigneeId");
        } catch (e) {
          if (e instanceof Error && "field" in e)
            (e as any).field = `tasks[${i}].${(e as any).field}`;
          throw e;
        }
      }
      const out = b.tasks.map((item: any) => makeTask(s, p, u.id, item, now));
      return answer({ items: out.map(taskView) }, 201);
    }
  }
  if (a[0] === "projects" && a[2] === "stats" && m === "GET")
    return stats(s, r, a[1]!);
  if (a[0] === "tasks" && a.length >= 2) {
    const u = auth(s, r),
      t = task(s, a[1]!, u.id),
      p = s.projects.find((x) => x.id === t.projectId)!;
    if (a.length === 2) {
      if (m === "GET") return answer(taskView(t), 200, etag(t));
      if (m === "PATCH") return update(s, r, t, p, u.id, b, now);
      if (m === "DELETE") {
        writeRole(s, p, u.id);
        if (u.id !== p.ownerId && u.id !== t.createdBy)
          fail(403, "forbidden", "Forbidden");
        remove(s, t);
        return answer(null, 204);
      }
    }
    if (a[2] === "transitions" && m === "POST")
      return transition(s, r, t, p, u.id, b, now);
    if (a[2] === "history" && m === "GET")
      return answer(s.history.get(t.key) || []);
    if (a[2] === "comments") return comments(s, r, t, u.id, m, b, now);
    if (a[2] === "dependencies") return dependencies(s, r, t, m, a, b, u.id);
  }
  return undefined;
}
function makeTask(
  s: State,
  p: Project,
  user: number,
  b: any,
  now: () => number,
) {
  validateTask(b);
  const assignee = b.assigneeId ?? null;
  if (assignee !== null && !s.members.get(p.id)?.has(assignee))
    fail(400, "validation", "Invalid assignee", "assigneeId");
  const n = ++p.seq,
    at = stamp(now),
    t: Task = {
      id: s.nextTask++,
      key: `${p.key}-${n}`,
      number: n,
      projectId: p.id,
      title: b.title.trim(),
      description: b.description ?? "",
      status: "open",
      priority: b.priority ?? "normal",
      labels: labels(b.labels ?? []),
      assigneeId: assignee,
      createdBy: user,
      createdAt: at,
      updatedAt: at,
      version: 1,
      deps: new Set(),
    };
  s.tasks.push(t);
  s.history.set(t.key, []);
  return t;
}
function create(
  s: State,
  p: Project,
  user: number,
  b: any,
  r: Request,
  now: () => number,
) {
  const key = r.headers.get("Idempotency-Key");
  const id = `${user}:${key}`;
  const encoded = JSON.stringify(b),
    old = key ? s.idem.get(id) : undefined;
  if (old && now() - old.at < 86400000) {
    if (old.body !== encoded)
      fail(422, "idempotency_mismatch", "Idempotency key reused");
    return answer(old.reply, 201, {
      "Idempotent-Replayed": "true",
      ETag: '"1"',
    });
  }
  const t = makeTask(s, p, user, b, now),
    out = taskView(t);
  if (key) s.idem.set(id, { at: now(), body: encoded, reply: out });
  return answer(out, 201, etag(t));
}
function remove(s: State, t: Task) {
  s.tasks = s.tasks.filter((x) => x !== t);
  for (const x of s.tasks) x.deps.delete(t.key);
}
function update(
  s: State,
  r: Request,
  t: Task,
  p: Project,
  user: number,
  b: any,
  now: () => number,
) {
  writeRole(s, p, user);
  precondition(r, t);
  validateTask(b, true);
  if (
    "assigneeId" in b &&
    b.assigneeId !== null &&
    !s.members.get(p.id)?.has(b.assigneeId)
  )
    fail(400, "validation", "Invalid assignee", "assigneeId");
  const changes: any[] = [];
  for (const k of Object.keys(b).sort()) {
    const v = k === "labels" ? labels(b[k]) : b[k];
    const next = k === "title" ? v.trim() : v;
    if (JSON.stringify((t as any)[k]) !== JSON.stringify(next))
      changes.push([k, (t as any)[k], next]);
  }
  if (changes.length) {
    const at = stamp(now);
    for (const [k, from, to] of changes) {
      (t as any)[k] = to;
      s.history.get(t.key)!.push({ at, by: user, field: k, from, to });
    }
    t.version++;
    t.updatedAt = at;
  }
  return answer(taskView(t), 200, etag(t));
}
