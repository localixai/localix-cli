import type { State, Task } from './model';
import { stamp, visible } from './model';
import { bad, conflict, empty, etag, fail, forbidden, member, missing, ok,
  roleCanWrite, taskFor } from './http';
const priorities = ['high', 'normal', 'low'];
function cleanLabels(v: any) {
  if (!Array.isArray(v) || v.some(x => typeof x !== 'string' || !x.trim() ||
    x.trim().length > 30 || /\s/.test(x.trim()))) return null;
  return [...new Set(v.map((x: string) => x.trim().toLowerCase()))].sort();
}
function validate(s: State, p: number, b: any, prefix = ''): string | undefined {
  const field = (x: string) => prefix ? `${prefix}.${x}` : x;
  if (typeof b?.title !== 'string' || !b.title.trim() || b.title.trim().length > 200) return field('title');
  if (b.description !== undefined && typeof b.description !== 'string') return field('description');
  if (b.priority !== undefined && !priorities.includes(b.priority)) return field('priority');
  if (b.labels !== undefined && cleanLabels(b.labels) === null) return field('labels');
  if (b.assigneeId !== undefined && b.assigneeId !== null &&
      !s.users.some(u => u.id === b.assigneeId && member(s, p, u.id))) return field('assigneeId');
  return undefined;
}
function makeTask(s: State, p: any, user: number, b: any): Task {
  const number = s.tasks.filter(t => t.projectId === p.id).length + 1;
  const time = stamp(s);
  const t: Task = { id: ++s.ids.task, key: `${p.key}-${number}`, number, projectId: p.id,
    title: b.title.trim(), description: b.description ?? '', status: 'open',
    priority: b.priority ?? 'normal', labels: b.labels === undefined ? [] : cleanLabels(b.labels)!,
    assigneeId: b.assigneeId ?? null, createdBy: user, createdAt: time, updatedAt: time, version: 1 };
  s.tasks.push(t); s.histories.set(t.id, []); s.comments.set(t.id, []); s.deps.set(t.id, new Set());
  return t;
}
function list(s: State, p: any, url: URL, user: number) {
  const q = url.searchParams;
  const status = q.get('status'), sort = q.get('sort') ?? 'created', assignee = q.get('assignee');
  const limitText = q.get('limit') ?? '20', limit = Number(limitText);
  if (status && !['open', 'in_progress', 'done'].includes(status)) return bad('status');
  if (!['created', '-created', 'priority'].includes(sort)) return bad('sort');
  if (!/^\d+$/.test(limitText) || limit < 1 || limit > 100) return bad('limit');
  if (assignee && assignee !== 'none' && assignee !== 'me' && !/^\d+$/.test(assignee)) return bad('assignee');
  let items = s.tasks.filter(t => t.projectId === p.id && !t.deleted);
  if (status) items = items.filter(t => t.status === status);
  if (assignee === 'none') items = items.filter(t => t.assigneeId === null);
  if (assignee === 'me') items = items.filter(t => t.assigneeId === user);
  if (assignee && /^\d+$/.test(assignee)) items = items.filter(t => t.assigneeId === Number(assignee));
  const label = q.get('label')?.toLowerCase(); if (label) items = items.filter(t => t.labels.includes(label));
  const term = q.get('q')?.toLowerCase();
  if (term) items = items.filter(t => `${t.title} ${t.description}`.toLowerCase().includes(term));
  items.sort((a, b) => sort === '-created' ? b.number - a.number : sort === 'priority' ?
    priorities.indexOf(a.priority) - priorities.indexOf(b.priority) || a.number - b.number : a.number - b.number);
  let offset = 0; const cursor = q.get('cursor');
  if (cursor) { try { offset = Number(atob(cursor)); if (!Number.isInteger(offset) || offset < 0) throw 0; }
    catch { return bad('cursor'); } }
  const page = items.slice(offset, offset + limit);
  return ok({ items: page.map(visible),
    nextCursor: offset + limit < items.length ? btoa(String(offset + limit)) : null });
}
export function taskRoute(s: State, method: string, path: string, url: URL, user: number | undefined,
  body: any, headers: Headers): any | undefined {
  const projectMatch = path.match(/^\/projects\/(\d+)\/(tasks|stats)(?:\/(bulk))?$/);
  if (projectMatch) {
    if (!user) return fail(401, 'unauthorized', 'Sign in required');
    const p = s.projects.find(x => x.id === Number(projectMatch[1]));
    if (!p || !member(s, p.id, user)) return missing();
    if (projectMatch[2] === 'stats') {
      if (method !== 'GET') return undefined;
      const items = s.tasks.filter(t => t.projectId === p.id && !t.deleted);
      const counts = (field: string, vals: string[]) => Object.fromEntries(vals.map(v => [v,
        items.filter(t => (t as any)[field] === v).length]));
      const byAssignee = new Map<number, number>();
      for (const t of items) if (t.assigneeId !== null) byAssignee.set(t.assigneeId,
        (byAssignee.get(t.assigneeId) ?? 0) + 1);
      return ok({ total: items.length, byStatus: counts('status', ['open', 'in_progress', 'done']),
        byPriority: counts('priority', ['high', 'normal', 'low']),
        byAssignee: [...byAssignee].map(([userId, count]) => ({ userId, count }))
          .sort((a, b) => b.count - a.count || a.userId - b.userId) });
    }
    if (projectMatch[3] === 'bulk' && method === 'POST') {
      if (!roleCanWrite(s, p.id, user)) return forbidden();
      const input = body?.tasks;
      if (!Array.isArray(input) || input.length < 1 || input.length > 50) return bad('tasks');
      for (let i = 0; i < input.length; i++) {
        const field = validate(s, p.id, input[i], `tasks[${i}]`); if (field) return bad(field);
      }
      return ok({ items: input.map((b: any) => visible(makeTask(s, p, user, b))) }, 201);
    }
    if (method === 'GET') return list(s, p, url, user);
    if (method === 'POST') {
      if (!roleCanWrite(s, p.id, user)) return forbidden();
      const key = headers.get('Idempotency-Key');
      const storeKey = key ? `${user}:${key}` : '';
      const cached = storeKey ? s.idempotency.get(storeKey) : undefined;
      if (cached && cached.expires > s.now()) {
        if (JSON.stringify(cached.body) !== JSON.stringify(body)) {
          return fail(422, 'idempotency_mismatch', 'Key reused');
        }
        return ok(cached.reply, 201, { ETag: '"1"', 'Idempotent-Replayed': 'true' });
      }
      const invalid = validate(s, p.id, body); if (invalid) return bad(invalid);
      const task = makeTask(s, p, user, body); const reply = visible(task);
      if (storeKey) s.idempotency.set(storeKey, { body, reply, expires: s.now() + 86400000 });
      return ok(reply, 201, etag(task));
    }
  }
  const mt = path.match(/^\/tasks\/([^/]+)(?:\/(history|comments|transitions|dependencies)(?:\/([^/]+))?)?$/);
  if (!mt) return undefined;
  if (!user) return fail(401, 'unauthorized', 'Sign in required');
  const task = taskFor(s, mt[1]!); if (!task || !member(s, task.projectId, user)) return missing();
  const sub = mt[2];
  if (!sub) {
    if (method === 'GET') return ok(visible(task), 200, etag(task));
    if (method === 'DELETE') {
      if (!roleCanWrite(s, task.projectId, user)) return forbidden();
      if (task.createdBy !== user && member(s, task.projectId, user) !== 'owner') return forbidden();
      task.deleted = true;
      for (const deps of s.deps.values()) deps.delete(task.id);
      return empty();
    }
    if (method === 'PATCH') {
      if (!roleCanWrite(s, task.projectId, user)) return forbidden();
      const expected = headers.get('If-Match');
      if (expected && expected !== `"${task.version}"`) return fail(412, 'precondition_failed', 'Task changed');
      const editable = ['title', 'description', 'priority', 'labels', 'assigneeId'];
      for (const k of Object.keys(body ?? {})) {
        if (!editable.includes(k)) return bad(k);
      }
      const invalid = validate(s, task.projectId, { ...task, ...body, title: body?.title ?? task.title });
      if (invalid) return bad(invalid);
      const changes: any[] = [];
      for (const k of ['assigneeId', 'labels', 'priority', 'title', 'description']) if (k in (body ?? {})) {
        const value = k === 'title' ? body[k].trim() : k === 'labels' ? cleanLabels(body[k]) : body[k];
        const from = (task as any)[k];
        if (JSON.stringify(from) !== JSON.stringify(value)) changes.push({ field: k, from, to: value });
      }
      if (changes.length) {
        const at = stamp(s);
        for (const c of changes) {
          (task as any)[c.field] = c.to;
          s.histories.get(task.id)!.push({ at, by: user, ...c });
        }
        task.updatedAt = at; task.version++;
      }
      return ok(visible(task), 200, etag(task));
    }
  }
  if (sub === 'history' && method === 'GET') return ok(s.histories.get(task.id) ?? []);
  if (sub === 'transitions') {
    if (method !== 'POST') return undefined;
    if (!roleCanWrite(s, task.projectId, user)) return forbidden();
    const expected = headers.get('If-Match');
    if (expected && expected !== `"${task.version}"`) return fail(412, 'precondition_failed', 'Task changed');
    if (!['open', 'in_progress', 'done'].includes(body?.to)) return bad('to');
    const valid = (task.status === 'open' && body.to === 'in_progress') ||
      (task.status === 'in_progress' && ['open', 'done'].includes(body.to)) ||
      (task.status === 'done' && body.to === 'open');
    if (!valid) return conflict('invalid_transition');
    if (body.to === 'done' && [...(s.deps.get(task.id) ?? [])].some(id =>
      s.tasks.some(t => t.id === id && !t.deleted && t.status !== 'done'))) return conflict('blocked');
    const from = task.status, at = stamp(s); task.status = body.to; task.updatedAt = at; task.version++;
    s.histories.get(task.id)!.push({ at, by: user, field: 'status', from, to: body.to });
    return ok(visible(task), 200, etag(task));
  }
  if (sub === 'comments') {
    const comments = s.comments.get(task.id)!;
    if (method === 'GET') return ok(comments);
    if (method === 'POST') {
      if (typeof body?.body !== 'string' || !body.body.trim() || body.body.length > 2000) return bad('body');
      const c = { id: ++s.ids.comment, authorId: user, body: body.body, createdAt: stamp(s) };
      comments.push(c); return ok(c, 201);
    }
  }
  if (sub === 'dependencies') {
    const deps = s.deps.get(task.id)!;
    if (method === 'GET') {
      const keys = (ids: number[]) => ids.map(id => s.tasks.find(t => t.id === id)?.key)
        .filter(Boolean).sort((a, b) => Number(a!.split('-')[1]) - Number(b!.split('-')[1]));
      const blocks = [...s.deps].filter(([, set]) => set.has(task.id))
        .map(([id]) => id).filter(id => s.tasks.some(t => t.id === id && !t.deleted));
      return ok({ key: task.key, dependsOn: keys([...deps]), blocks: keys(blocks) });
    }
    if (mt[3] && method === 'DELETE') {
      const other = taskFor(s, mt[3]); if (!other || !deps.has(other.id)) return missing();
      deps.delete(other.id); return empty();
    }
    if (method === 'POST') {
      if (!roleCanWrite(s, task.projectId, user)) return forbidden();
      const other = taskFor(s, body?.dependsOn);
      if (!other || other.id === task.id || other.projectId !== task.projectId) return bad('dependsOn');
      if (deps.has(other.id)) return conflict();
      const reaches = (from: number, target: number, seen = new Set<number>()): boolean => {
        if (from === target) return true; if (seen.has(from)) return false; seen.add(from);
        return [...(s.deps.get(from) ?? [])].some(next => reaches(next, target, seen));
      };
      if (reaches(other.id, task.id)) return conflict('dependency_cycle');
      deps.add(other.id);
      return ok({ key: task.key, dependsOn: [...deps].map(id => s.tasks.find(t => t.id === id)!.key)
        .sort((a, b) => Number(a.split('-')[1]) - Number(b.split('-')[1])), blocks: [] }, 201);
    }
  }
  return undefined;
}
