import type { Ctx, Project } from './model';
import { fail, json, taskView } from './http';
export function listTasks(c: Ctx, p: Project, u: { id: number }, url: URL) {
  const q = url.searchParams, valid = (v: string | null, opts: string[], f: string) => {
    if (v !== null && !opts.includes(v)) fail(400, 'validation', `${f} is invalid`, f);
  };
  valid(q.get('status'), ['open', 'in_progress', 'done'], 'status');
  valid(q.get('sort'), ['created', '-created', 'priority'], 'sort');
  const assignee = q.get('assignee');
  if (assignee !== null && assignee !== 'me' && assignee !== 'none' &&
      !/^\d+$/.test(assignee)) fail(400, 'validation', 'assignee is invalid', 'assignee');
  let limit = 20;
  if (q.has('limit')) {
    const n = Number(q.get('limit'));
    if (!Number.isInteger(n) || n < 1 || n > 100) fail(400, 'validation', 'limit is invalid', 'limit');
    limit = n;
  }
  let items = c.s.tasks.filter(t => t.projectId === p.id);
  if (q.has('status')) items = items.filter(t => t.status === q.get('status'));
  if (assignee === 'me') items = items.filter(t => t.assigneeId === u.id);
  else if (assignee === 'none') items = items.filter(t => t.assigneeId === null);
  else if (assignee) items = items.filter(t => t.assigneeId === Number(assignee));
  if (q.has('label')) items = items.filter(t => t.labels.includes(q.get('label')!.trim().toLowerCase()));
  if (q.has('q')) {
    const s = q.get('q')!.toLowerCase();
    items = items.filter(t => `${t.title} ${t.description}`.toLowerCase().includes(s));
  }
  const sort = q.get('sort') || 'created';
  items.sort((a, b) => sort === '-created' ? b.number - a.number : sort === 'priority' ?
    ['high', 'normal', 'low'].indexOf(a.priority) -
      ['high', 'normal', 'low'].indexOf(b.priority) || a.number - b.number : a.number - b.number);
  let offset = 0;
  if (q.has('cursor')) {
    try {
      offset = Number(atob(q.get('cursor')!));
      if (!Number.isInteger(offset) || offset < 0) throw 0;
    } catch { fail(400, 'validation', 'cursor is invalid', 'cursor'); }
  }
  const page = items.slice(offset, offset + limit), more = offset + limit < items.length;
  return json({ items: page.map(t => taskView(c, t)), nextCursor: more ? btoa(String(offset + limit)) : null });
}
