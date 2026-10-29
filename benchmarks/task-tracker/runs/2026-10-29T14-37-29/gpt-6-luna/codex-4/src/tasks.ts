import type { Ctx, Project, Task } from './model';
import { next } from './model';
import { listTasks } from './task-list';
import { body, checkVersion, etag, fail, json, need, project, role, task, taskView, user } from './http';
function fields(b: Record<string, any>, fieldPrefix = '') {
  const out: Record<string, any> = {};
  for (const k of Object.keys(b)) {
    const f = fieldPrefix ? `${fieldPrefix}.${k}` : k;
    if (!['title', 'description', 'priority', 'labels', 'assigneeId'].includes(k)) {
      fail(400, 'validation', `${f} is not allowed`, f);
    }
  }
  if ('title' in b) {
    const field = fieldPrefix ? `${fieldPrefix}.title` : 'title';
    need(typeof b.title === 'string' && b.title.trim().length > 0 &&
      b.title.trim().length <= 200, field);
    out.title = b.title.trim();
  }
  if ('description' in b) {
    need(typeof b.description === 'string', fieldPrefix ? `${fieldPrefix}.description` : 'description');
    out.description = b.description;
  }
  if ('priority' in b) {
    need(['low', 'normal', 'high'].includes(b.priority), fieldPrefix ? `${fieldPrefix}.priority` : 'priority');
    out.priority = b.priority;
  }
  if ('labels' in b) {
    need(Array.isArray(b.labels) && b.labels.every((x: unknown) =>
      typeof x === 'string' && /^[^\s]{1,30}$/.test(x.trim())),
      fieldPrefix ? `${fieldPrefix}.labels` : 'labels');
    out.labels = [...new Set(b.labels.map((x: string) => x.trim().toLowerCase()))].sort();
  }
  if ('assigneeId' in b) out.assigneeId = b.assigneeId;
  return out;
}
function create(c: Ctx, p: Project, u: { id: number }, b: Record<string, any>, prefix = ''): Task {
  const f = fields(b, prefix); need(typeof f.title === 'string', 'title');
  if (f.assigneeId !== undefined && f.assigneeId !== null) {
      need(c.s.members.get(p.id)?.has(f.assigneeId), 'assigneeId');
    }
  const n = (c.s.ids[`task-${p.id}`] || 0) + 1; c.s.ids[`task-${p.id}`] = n;
  const t: Task = { id: next(c, 'task'), number: n, projectId: p.id,
    title: f.title, description: f.description ?? '', status: 'open', priority: f.priority ?? 'normal',
    labels: f.labels ?? [], assigneeId: f.assigneeId ?? null, createdBy: u.id,
    createdAt: new Date(c.now()).toISOString(), updatedAt: new Date(c.now()).toISOString(), version: 1 };
  c.s.tasks.push(t); c.s.histories.set(t.id, []); c.s.comments.set(t.id, []); c.s.deps.set(t.id, new Set()); return t;
}
export async function taskRoute(c: Ctx, req: Request, path: string, url: URL) {
  const m = req.method;
  const collection = path.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if (collection) {
    const u = user(c, req), p = project(c, collection[1]!, u);
    if (m === 'POST') {
      role(c, p, u, true); const b = await body(req);
      if (collection[2]) {
        need(Array.isArray(b.tasks) && b.tasks.length >= 1 && b.tasks.length <= 50, 'tasks');
        const inputs: Record<string, any>[] = b.tasks.map((x: any, i: number) => {
          const item = x && typeof x === 'object' && !Array.isArray(x) ? x : {};
          const f = fields(item, `tasks[${i}]`);
          if (f.assigneeId != null) need(c.s.members.get(p.id)?.has(f.assigneeId),
            `tasks[${i}].assigneeId`);
          need(typeof f.title === 'string', `tasks[${i}].title`);
          return item;
        });
        const all = inputs.map((x, i) => create(c, p, u, x, `tasks[${i}]`));
        return json({ items: all.map(t => taskView(c, t)) }, 201);
      }
      const key = req.headers.get('Idempotency-Key'), idemKey = key ? `${u.id}:${p.id}:${key}` : '';
      const old = idemKey ? c.s.idem.get(idemKey) : undefined, raw = JSON.stringify(b);
      if (old && c.now() - old.at < 86400000) {
        if (old.body !== raw) fail(422, 'idempotency_mismatch', 'key used with another body');
        return json(taskView(c, old.task), 201, { ...etag(old.task), 'Idempotent-Replayed': 'true' });
      }
      const t = create(c, p, u, b); if (idemKey) c.s.idem.set(idemKey, { at: c.now(), body: raw, task: t });
      return json(taskView(c, t), 201, etag(t));
    }
    if (m === 'GET' && !collection[2]) return listTasks(c, p, u, url);
  }
  const delDep = path.match(/^\/tasks\/([^/]+)\/dependencies\/([^/]+)$/);
  if (delDep && m === 'DELETE') {
    const u = user(c, req), t = task(c, delDep[1]!, u);
    const p = c.s.projects.find(x => x.id === t.projectId)!; role(c, p, u, true);
    const dep = c.s.tasks.find(x => taskView(c, x).key === delDep[2]);
    if (!dep || !c.s.deps.get(t.id)?.has(dep.id)) {
      return fail(404, 'not_found', 'dependency not found');
    }
    c.s.deps.get(t.id)!.delete(dep.id); return json(null, 204);
  }
  const match = path.match(/^\/tasks\/([^/]+)(?:\/(history|comments|transitions|dependencies))?$/);
  if (!match) return null;
  const u = user(c, req), t = task(c, match[1]!, u), p = c.s.projects.find(x => x.id === t.projectId)!;
  const sub = match[2];
  if (!sub && m === 'GET') return json(taskView(c, t), 200, etag(t));
  if (!sub && m === 'PATCH') {
    role(c, p, u, true); checkVersion(req, t); const b = await body(req), f = fields(b);
    if (f.assigneeId !== undefined && f.assigneeId !== null) {
      need(c.s.members.get(p.id)?.has(f.assigneeId), 'assigneeId');
    }
    const changed: string[] = [];
    for (const k of Object.keys(f).sort()) {
      const before = (t as any)[k], after = f[k];
      if (JSON.stringify(before) !== JSON.stringify(after)) {
        c.s.histories.get(t.id)!.push({ at: new Date(c.now()).toISOString(),
          by: u.id, field: k, from: before, to: after });
        (t as any)[k] = after; changed.push(k);
      }
    }
    if (changed.length) { t.version++; t.updatedAt = new Date(c.now()).toISOString(); }
    return json(taskView(c, t), 200, etag(t));
  }
  if (!sub && m === 'DELETE') {
    const r = role(c, p, u);
    if (r !== 'owner' && t.createdBy !== u.id) {
      fail(403, 'forbidden', 'cannot delete this task');
    }
    removeTask(c, t); return json(null, 204);
  }
  if (sub === 'history' && m === 'GET') return json(c.s.histories.get(t.id) || []);
  if (sub === 'transitions' && m === 'POST') return transition(c, req, t, p, u);
  if (sub === 'comments') return comments(c, req, t, u, m);
  if (sub === 'dependencies') return dependencies(c, req, t, p, u, m);
  return null;
}
function transition(c: Ctx, req: Request, t: Task, p: Project, u: { id: number }) {
  role(c, p, u, true); checkVersion(req, t);
  return body(req).then(b => {
    need(['open', 'in_progress', 'done'].includes(b.to), 'to');
    const allowed = t.status === 'open' ? b.to === 'in_progress' :
      t.status === 'in_progress' ? ['open', 'done'].includes(b.to) : b.to === 'open';
    if (!allowed) fail(409, 'invalid_transition', 'transition is not allowed');
    if (b.to === 'done' && [...(c.s.deps.get(t.id) || [])].some(id =>
      c.s.tasks.find(x => x.id === id)?.status !== 'done')) {
      fail(409, 'blocked', 'task has unfinished dependencies');
    }
    c.s.histories.get(t.id)!.push({ at: new Date(c.now()).toISOString(),
      by: u.id, field: 'status', from: t.status, to: b.to });
    t.status = b.to; t.version++; t.updatedAt = new Date(c.now()).toISOString();
    return json(taskView(c, t), 200, etag(t));
  });
}
function comments(c: Ctx, req: Request, t: Task, u: { id: number }, m: string) {
  if (m === 'GET') return json(c.s.comments.get(t.id) || []);
  if (m === 'POST') return body(req).then(b => {
    need(typeof b.body === 'string' && b.body.trim().length > 0 && b.body.length <= 2000, 'body');
    const list = c.s.comments.get(t.id)!;
    const item = { id: list.length + 1, authorId: u.id, body: b.body.trim(),
      createdAt: new Date(c.now()).toISOString() };
    list.push(item); return json(item, 201);
  });
  return null;
}
function dependencies(c: Ctx, req: Request, t: Task, p: Project, u: { id: number }, m: string) {
  const ds = c.s.deps.get(t.id)!;
  const sorted = (ids: number[]) => ids.map(id => c.s.tasks.find(x => x.id === id)!)
    .filter(Boolean).sort((a, b) => a.number - b.number).map(x => taskView(c, x).key);
  if (m === 'GET') return json({ key: taskView(c, t).key,
    dependsOn: sorted([...ds]),
    blocks: sorted([...c.s.deps].filter(([, set]) => set.has(t.id)).map(([id]) => id)) });
  role(c, p, u, true);
  if (m === 'POST') return body(req).then(b => {
    const on = typeof b.dependsOn === 'string' ? c.s.tasks.find(x => taskView(c, x).key === b.dependsOn) : undefined;
    need(on && on.id !== t.id && on.projectId === t.projectId, 'dependsOn');
    if (ds.has(on!.id)) fail(409, 'conflict', 'dependency already exists');
    const reaches = (from: number, target: number, seen = new Set<number>()): boolean => {
      if (from === target) return true; if (seen.has(from)) return false; seen.add(from);
      return [...(c.s.deps.get(from) || [])].some(id => reaches(id, target, seen));
    };
    if (reaches(on!.id, t.id)) fail(409, 'dependency_cycle', 'dependency would make a cycle');
    ds.add(on!.id);
    return json({ key: taskView(c, t).key, dependsOn: sorted([...ds]),
      blocks: sorted([...c.s.deps].filter(([, set]) => set.has(t.id)).map(([id]) => id)) }, 201);
  });
  return null;
}
function removeTask(c: Ctx, t: Task) {
  c.s.tasks = c.s.tasks.filter(x => x.id !== t.id); c.s.deps.delete(t.id);
  for (const deps of c.s.deps.values()) deps.delete(t.id);
}
