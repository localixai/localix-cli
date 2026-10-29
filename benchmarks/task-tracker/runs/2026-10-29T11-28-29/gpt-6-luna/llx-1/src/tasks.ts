import { fail, obj, projectAccess, taskAccess, valid, writeRole } from "./common";
import { iso, taskOut, type State, type Task } from "./model";

function normalize(b: Record<string, any>, s: State, pid: number, field = "") {
  valid(typeof b.title === "string" && b.title.trim().length > 0 && b.title.trim().length <= 200, `${field}title`);
  const labels = b.labels ?? [];
  valid(Array.isArray(labels) && labels.every(x => typeof x === "string" &&
    x.trim().length > 0 && x.trim().length <= 30 && !/\s/.test(x.trim())), `${field}labels`);
  valid(b.description === undefined || typeof b.description === "string", `${field}description`);
  valid(b.priority === undefined || ["low", "normal", "high"].includes(b.priority), `${field}priority`);
  const assignee = b.assigneeId ?? null;
  valid(assignee === null || s.members.get(pid)?.has(assignee), `${field}assigneeId`);
  return { title: b.title.trim(), description: b.description ?? "", priority: b.priority ?? "normal",
    labels: [...new Set(labels.map((x: string) => x.trim().toLowerCase()))].sort(), assignee };
}
export function create(s: State, pid: number, user: number, b: Record<string, any>): Task {
  const v = normalize(b, s, pid);
  const p = s.projects.find(x => x.id === pid)!;
  const number = (s.counters.get(p.key) ?? 0) + 1; s.counters.set(p.key, number);
  const at = iso(s), t: Task = { id: s.nextTask++, key: `${p.key}-${number}`, number, projectId: pid,
    ...v, assigneeId: v.assignee, status: "open", createdBy: user, createdAt: at, updatedAt: at,
    version: 1, history: [], comments: [], deps: new Set() };
  s.tasks.push(t); return t;
}
function version(req: Request, t: Task) {
  const match = req.headers.get("If-Match");
  if (match && match !== `"${t.version}"`) fail(412, "precondition_failed");
}
export async function taskRoute(s: State, req: Request, url: URL, path: string, user: number) {
  const bulk = /^\/projects\/(\d+)\/tasks\/bulk$/.exec(path);
  const listing = /^\/projects\/(\d+)\/tasks$/.exec(path);
  if (bulk || listing) {
    const pid = Number((bulk || listing)![1]); projectAccess(s, pid, user);
    if (bulk && req.method === "POST") {
      writeRole(s, { projectId: pid }, user);
      const b = obj(await req.json().catch(() => fail(400, "validation")));
      valid(Array.isArray(b.tasks) && b.tasks.length >= 1 && b.tasks.length <= 50, "tasks");
      const items = b.tasks.map((x: any, i: number) => {
        try { normalize(obj(x), s, pid); } catch (e) {
          if (e instanceof Error && "field" in e) (e as any).field = `tasks[${i}].${(e as any).field}`;
          throw e;
        }
        return taskOut(create(s, pid, user, x));
      });
      return [201, { items }];
    }
    if (listing && req.method === "GET") {
      let items = s.tasks.filter(t => t.projectId === pid);
      const status = url.searchParams.get("status"), label = url.searchParams.get("label");
      const q = url.searchParams.get("q")?.toLowerCase();
      if (status) items = items.filter(t => t.status === status);
      if (label) items = items.filter(t => t.labels.includes(label.toLowerCase()));
      if (q) items = items.filter(t => `${t.title} ${t.description}`.toLowerCase().includes(q));
      const limit = Number(url.searchParams.get("limit") ?? 20);
      const cursor = Number(url.searchParams.get("cursor") ?? 0);
      items = items.slice().sort((a, b) => a.number - b.number);
      const page = items.slice(cursor, cursor + limit);
      return [200, { items: page.map(taskOut), nextCursor: cursor + limit < items.length ?
        String(cursor + limit) : null }];
    }
    return null;
  }
  const match = /^\/tasks\/([^/]+)(?:\/(transitions|history|comments|dependencies)(?:\/([^/]+))?)?$/.exec(path);
  if (!match) return null;
  const t = taskAccess(s, match[1]!, user)!, sub = match[2], method = req.method;
  if (!sub) {
    if (method === "GET") return [200, taskOut(t)];
    if (method === "DELETE") {
      writeRole(s, t, user);
      if (t.createdBy !== user && s.projects.find(p => p.id === t.projectId)?.ownerId !== user) fail(403, "forbidden");
      s.tasks = s.tasks.filter(x => x !== t);
      s.tasks.forEach(x => x.deps.delete(t.key)); return [204, null];
    }
    if (method === "PATCH") {
      writeRole(s, t, user); version(req, t);
      const b = obj(await req.json().catch(() => fail(400, "validation")));
      const allowed = ["title", "description", "priority", "labels", "assigneeId"];
      for (const k of Object.keys(b)) valid(allowed.includes(k), k);
      const next = normalize({ ...t, ...b }, s, t.projectId);
      const changes: any[] = [];
      for (const k of allowed.sort()) if (JSON.stringify((t as any)[k] ?? (k === "assigneeId" ? null : undefined)) !==
        JSON.stringify((next as any)[k === "assigneeId" ? "assignee" : k])) {
        changes.push({ at: iso(s), by: user, field: k, from: (t as any)[k], to: (next as any)[k === "assigneeId" ? "assignee" : k] });
      }
      Object.assign(t, next, { assigneeId: next.assignee });
      if (changes.length) { t.version++; t.updatedAt = iso(s); t.history.push(...changes); }
      return [200, taskOut(t)];
    }
  }
  if (sub === "history" && method === "GET") return [200, t.history];
  if (sub === "comments") {
    if (method === "GET") return [200, t.comments];
    if (method === "POST") {
      const b = obj(await req.json().catch(() => fail(400, "validation")));
      valid(typeof b.body === "string" && b.body.trim().length > 0 && b.body.length <= 2000, "body");
      const c = { id: s.nextComment++, authorId: user, body: b.body.trim(), createdAt: iso(s) };
      t.comments.push(c); return [201, c];
    }
  }
  if (sub === "transitions" && method === "POST") {
    writeRole(s, t, user); version(req, t);
    const b = obj(await req.json().catch(() => fail(400, "validation")));
    valid(["open", "in_progress", "done"].includes(b.to), "to");
    const allowed = (t.status === "open" && b.to === "in_progress") ||
      (t.status === "in_progress" && ["open", "done"].includes(b.to)) ||
      (t.status === "done" && b.to === "open");
    if (!allowed) fail(409, "invalid_transition");
    const from = t.status; t.status = b.to; t.version++; t.updatedAt = iso(s);
    t.history.push({ at: t.updatedAt, by: user, field: "status", from, to: b.to });
    return [200, taskOut(t)];
  }
  return null;
}
