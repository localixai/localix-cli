import type { Store, Project, Task } from "./model";
import { iso } from "./model";
import { bad, fail, obj, reply } from "./http";
import { patchFields } from "./validation";
const view = (t: Task) => {
  const { history, comments, depends, ...out } = t; return out;
};
const canSee = (s: Store, t: Task, uid: number) => {
  const p = s.projects.find((x) => x.id === t.projectId);
  if (!p || !p.members.has(uid)) fail(404, "not_found", "Task not found");
  return p;
};
const get = (s: Store, key: string, uid: number) => {
  const t = s.tasks.find((x) => x.key === key);
  if (!t) fail(404, "not_found", "Task not found");
  return { t, p: canSee(s, t, uid) };
};
const writer = (p: Project, uid: number) => {
  if (!["owner", "member"].includes(p.members.get(uid) ?? "")) fail(403, "forbidden", "Forbidden");
};
function checkMatch(r: Request, t: Task) {
  const tag = r.headers.get("If-Match");
  if (tag && tag !== `"${t.version}"`) fail(412, "precondition_failed", "Task has changed");
}
function addHistory(s: Store, t: Task, uid: number, field: string, from: any, to: any) {
  if (JSON.stringify(from) === JSON.stringify(to)) return false;
  (t as any)[field] = to; t.history.push({ at: iso(s), by: uid, field, from, to });
  return true;
}
export async function taskRoute(s: Store, r: Request, u: URL, uid: number, key: string,
  suffix: string): Promise<Response> {
  const { t, p } = get(s, key, uid), method = r.method;
  if (!suffix && method === "GET") return reply(view(t), 200, { ETag: `"${t.version}"` });
  if (!suffix && method === "PATCH") {
    writer(p, uid); checkMatch(r, t);
    const b = await obj(r), changes = patchFields(b, t, p, s);
    let changed = false;
    for (const field of Object.keys(changes).sort()) {
      const from = (t as any)[field], to = (changes as any)[field];
      changed = addHistory(s, t, uid, field, from, to) || changed;
    }
    if (changed) { t.version++; t.updatedAt = iso(s); }
    return reply(view(t), 200, { ETag: `"${t.version}"` });
  }
  if (!suffix && method === "DELETE") {
    if (p.ownerId !== uid && t.createdBy !== uid) fail(403, "forbidden", "Forbidden");
    s.tasks.splice(s.tasks.indexOf(t), 1);
    for (const x of s.tasks) x.depends.delete(key);
    return reply(null, 204);
  }
  if (suffix === "history" && method === "GET") return reply(t.history);
  if (suffix === "comments") return comments(s, r, t, uid, method);
  if (suffix === "dependencies") return dependencies(s, r, t, uid, method, null);
  const dep = suffix.match(/^dependencies\/([A-Z]+-\d+)$/);
  if (dep) return dependencies(s, r, t, uid, method, dep[1]!);
  if (suffix === "transitions" && method === "POST") {
    writer(p, uid); checkMatch(r, t);
    const b = await obj(r);
    if (!["open", "in_progress", "done"].includes(b.to)) bad("to");
    const valid = (t.status === "open" && b.to === "in_progress") ||
      (t.status === "in_progress" && ["open", "done"].includes(b.to)) ||
      (t.status === "done" && b.to === "open");
    if (!valid) fail(409, "invalid_transition", "Invalid transition");
    if (b.to === "done" && [...t.depends].some((k) =>
      s.tasks.some((x) => x.key === k && x.status !== "done"))) fail(409, "blocked", "Dependencies remain");
    addHistory(s, t, uid, "status", t.status, b.to);
    t.version++; t.updatedAt = iso(s);
    return reply(view(t), 200, { ETag: `"${t.version}"` });
  }
  fail(404, "not_found", "Not found");
}
async function comments(s: Store, r: Request, t: Task, uid: number, method: string) {
  if (method === "GET") return reply(t.comments);
  if (method !== "POST") fail(405, "method_not_allowed", "Method not allowed");
  const b = await obj(r), body = typeof b.body === "string" ? b.body.trim() : "";
  if (!body || body.length > 2000) bad("body");
  const comment = { id: t.comments.length + 1, authorId: uid, body, createdAt: iso(s) };
  t.comments.push(comment); return reply(comment, 201);
}
async function dependencies(s: Store, r: Request, t: Task, uid: number, method: string,
  other: string | null) {
  if (method === "GET") {
    const deps = [...t.depends].sort((a, b) => number(a) - number(b));
    const blocks = s.tasks.filter((x) => x.depends.has(t.key)).map((x) => x.key)
      .sort((a, b) => number(a) - number(b));
    return reply({ key: t.key, dependsOn: deps, blocks });
  }
  const p = canSee(s, t, uid); writer(p, uid);
  if (method === "POST" && !other) {
    const b = await obj(r), key = b.dependsOn;
    const candidate = typeof key === "string" ? s.tasks.find((x) => x.key === key) : null;
    if (!candidate || candidate.key === t.key || candidate.projectId !== t.projectId)
      bad("dependsOn");
    if (t.depends.has(key)) fail(409, "conflict", "Dependency exists");
    if (reachable(s, key, t.key)) fail(409, "dependency_cycle", "Dependency cycle");
    t.depends.add(key);
    const deps = [...t.depends].sort((a, b) => number(a) - number(b));
    return reply({ key: t.key, dependsOn: deps, blocks: [] }, 201);
  }
  if (method === "DELETE" && other) {
    if (!t.depends.delete(other)) fail(404, "not_found", "Dependency not found");
    return reply(null, 204);
  }
  fail(405, "method_not_allowed", "Method not allowed");
}
const number = (key: string) => Number(key.split("-").at(-1));
function reachable(s: Store, from: string, target: string, seen = new Set<string>()): boolean {
  if (from === target) return true;
  if (seen.has(from)) return false;
  seen.add(from);
  const t = s.tasks.find((x) => x.key === from);
  return !!t && [...t.depends].some((x) => reachable(s, x, target, seen));
}
export function listing(s: Store, p: Project, u: URL, uid: number) {
  let list = s.tasks.filter((t) => t.projectId === p.id);
  const q = u.searchParams;
  const status = q.get("status"), assignee = q.get("assignee"), label = q.get("label");
  const search = q.get("q")?.toLowerCase();
  if (status && !["open", "in_progress", "done"].includes(status)) bad("status");
  if (status) list = list.filter((t) => t.status === status);
  if (assignee) {
    if (!["me", "none"].includes(assignee) && !/^\d+$/.test(assignee)) bad("assignee");
    list = list.filter((t) => assignee === "none" ? t.assigneeId === null :
      t.assigneeId === (assignee === "me" ? uid : Number(assignee)));
  }
  if (label) list = list.filter((t) => t.labels.includes(label.toLowerCase()));
  if (search) list = list.filter((t) => `${t.title} ${t.description}`.toLowerCase().includes(search));
  const sort = q.get("sort") ?? "created";
  if (!["created", "-created", "priority"].includes(sort)) bad("sort");
  list.sort((a, b) => sort === "-created" ? b.number - a.number :
    sort === "priority" ? ["high", "normal", "low"].indexOf(a.priority) -
      ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number : a.number - b.number);
  const limitRaw = q.get("limit") ?? "20", limit = Number(limitRaw);
  if (!/^\d+$/.test(limitRaw) || limit < 1 || limit > 100) bad("limit");
  let offset = 0, cursor = q.get("cursor");
  if (cursor) {
    try {
      const parsed = JSON.parse(atob(cursor));
      if (parsed.sort !== sort || parsed.filter !== `${status}|${assignee}|${label}|${search}`) bad("cursor");
      offset = parsed.offset;
      if (!Number.isInteger(offset) || offset < 0) bad("cursor");
    } catch { bad("cursor"); }
  }
  const items = list.slice(offset, offset + limit);
  const nextCursor = offset + limit < list.length ?
    btoa(JSON.stringify({ offset: offset + limit, sort,
      filter: `${status}|${assignee}|${label}|${search}` })) : null;
  return reply({ items: items.map(view), nextCursor });
}
