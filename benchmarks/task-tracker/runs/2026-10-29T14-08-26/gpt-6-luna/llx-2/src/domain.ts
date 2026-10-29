import type { Store, Project, Task } from "./model";
import { iso } from "./model";
import { bad, fail, obj, reply } from "./http";
import { patchFields, taskFields } from "./validation";
import { listing, taskRoute } from "./task-routes";
const view = (t: Task) => {
  const { history, comments, depends, ...out } = t;
  return out;
};
const project = (s: Store, id: number, user: number) => {
  const p = s.projects.find((x) => x.id === id);
  if (!p || !p.members.has(user)) fail(404, "not_found", "Project not found");
  return p;
};
const task = (s: Store, key: string, user: number) => {
  const t = s.tasks.find((x) => x.key === key);
  if (!t) fail(404, "not_found", "Task not found");
  const p = project(s, t.projectId, user);
  return { t, p };
};
const write = (p: Project, user: number) => {
  if (!["owner", "member"].includes(p.members.get(user) ?? "")) fail(403, "forbidden", "Forbidden");
};
function make(s: Store, p: Project, uid: number, b: Record<string, any>): Task {
  const fields = taskFields(b, p, s), n = p.next++, at = iso(s);
  const t: Task = { id: s.tasks.length + 1, key: `${p.key}-${n}`, number: n,
    projectId: p.id, ...fields, status: "open", createdBy: uid, createdAt: at,
    updatedAt: at, version: 1, history: [], comments: [], depends: new Set() };
  s.tasks.push(t); return t;
}
function precondition(r: Request, t: Task) {
  const value = r.headers.get("If-Match");
  if (value && value !== `"${t.version}"`) fail(412, "precondition_failed", "Task has changed");
}
function save(s: Store, t: Task, uid: number, field: string, from: any, to: any) {
  if (JSON.stringify(from) === JSON.stringify(to)) return false;
  (t as any)[field] = to;
  t.history.push({ at: iso(s), by: uid, field, from, to });
  return true;
}
export async function domain(s: Store, r: Request, u: URL, uid: number): Promise<Response> {
  const path = u.pathname, method = r.method;
  if (path === "/projects" && method === "GET") return reply(s.projects.filter((p) =>
    p.members.has(uid)).map(({ members, next, ...p }) => p));
  if (path === "/projects" && method === "POST") {
    const b = await obj(r), name = typeof b.name === "string" ? b.name.trim() : "";
    if (!name || name.length > 100) bad("name");
    if (typeof b.key !== "string" || !/^[A-Z]{2,6}$/.test(b.key)) bad("key");
    if (s.projects.some((p) => p.key === b.key)) fail(409, "conflict", "Key taken", "key");
    const p: Project = { id: s.projects.length + 1, name, key: b.key, ownerId: uid,
      createdAt: iso(s), members: new Map([[uid, "owner"]]), next: 1 };
    s.projects.push(p); return reply(projectView(p), 201);
  }
  let m = path.match(/^\/projects\/(\d+)$/);
  if (m && method === "GET") return reply(projectView(project(s, Number(m[1]), uid)));
  m = path.match(/^\/projects\/(\d+)\/members(?:\/(\d+))?$/);
  if (m) return members(s, r, Number(m[1]), m[2] ? Number(m[2]) : null, uid, method);
  m = path.match(/^\/projects\/(\d+)\/(tasks|stats)(?:\/bulk)?$/);
  if (m) {
    const p = project(s, Number(m[1]), uid);
    if (m[2] === "stats" && method === "GET") return stats(s, p);
    if (m[2] === "tasks" && method === "POST") {
      write(p, uid);
      const b = await obj(r);
      if (path.endsWith("/bulk")) return bulk(s, p, uid, b);
      return create(s, r, p, uid, b);
    }
    if (m[2] === "tasks" && method === "GET") return listing(s, p, u, uid);
  }
  m = path.match(/^\/tasks\/([A-Z]+-\d+)(?:\/(.*))?$/);
  if (m) return taskRoute(s, r, u, uid, m[1]!, m[2] ?? "");
  fail(404, "not_found", "Not found");
}
function projectView(p: Project) {
  return { id: p.id, name: p.name, key: p.key, ownerId: p.ownerId, createdAt: p.createdAt };
}
async function members(s: Store, r: Request, id: number, member: number | null, uid: number,
  method: string) {
  const p = project(s, id, uid);
  if (method === "GET") return reply([...p.members].map(([userId, role]) => ({ userId, role }))
    .sort((a, b) => a.userId === p.ownerId ? -1 : b.userId === p.ownerId ? 1 : a.userId - b.userId));
  if (method === "POST") {
    if (p.ownerId !== uid) fail(403, "forbidden", "Forbidden");
    const b = await obj(r);
    if (!["member", "viewer"].includes(b.role)) bad("role");
    if (!Number.isInteger(b.userId) || !s.users.some((x) => x.id === b.userId)) bad("userId");
    if (p.members.has(b.userId)) fail(409, "conflict", "Already a member");
    p.members.set(b.userId, b.role); return reply({ userId: b.userId, role: b.role }, 201);
  }
  if (method === "DELETE" && member !== null) {
    if (p.ownerId !== uid) fail(403, "forbidden", "Forbidden");
    if (!p.members.has(member) || member === p.ownerId) fail(404, "not_found", "Member not found");
    p.members.delete(member);
    for (const t of s.tasks.filter((x) => x.projectId === id && x.assigneeId === member)) t.assigneeId = null;
    return reply(null, 204);
  }
  fail(405, "method_not_allowed", "Method not allowed");
}
async function create(s: Store, r: Request, p: Project, uid: number, b: Record<string, any>) {
  const key = r.headers.get("Idempotency-Key");
  const ik = key ? `${uid}:${key}` : "", signature = JSON.stringify([p.id, b]);
  const old = ik && s.idem.get(ik);
  if (old && s.now() - old.at < 86400000) {
    if (old.signature !== signature) fail(422, "idempotency_mismatch", "Key reused");
    return reply(old.result, 201, { "Idempotent-Replayed": "true", ETag: '"1"' });
  }
  const t = make(s, p, uid, b), result = view(t);
  if (ik) s.idem.set(ik, { at: s.now(), signature, result });
  return reply(result, 201, { ETag: '"1"' });
}
async function bulk(s: Store, p: Project, uid: number, b: Record<string, any>) {
  if (!Array.isArray(b.tasks) || b.tasks.length < 1 || b.tasks.length > 50) bad("tasks");
  for (let i = 0; i < b.tasks.length; i++) {
    try { taskFields(b.tasks[i] ?? {}, p, s); }
    catch (e: any) { if (e.field) e.field = `tasks[${i}].${e.field}`; throw e; }
  }
  return reply({ items: b.tasks.map((x: any) => view(make(s, p, uid, x))) }, 201);
}
function stats(s: Store, p: Project) {
    const all = s.tasks.filter((t) => t.projectId === p.id);
  const buckets = (field: string, names: string[]) => Object.fromEntries(names.map((n) =>
    [n, all.filter((t) => (t as any)[field] === n).length]));
  const assignees = new Map<number, number>();
  for (const t of all) if (t.assigneeId !== null) assignees.set(t.assigneeId,
    (assignees.get(t.assigneeId) ?? 0) + 1);
  const byAssignee = [...assignees].map(([userId, count]) => ({ userId, count }))
    .sort((a, b) => b.count - a.count || a.userId - b.userId);
  return reply({ total: all.length, byStatus: buckets("status", ["open", "in_progress", "done"]),
    byPriority: buckets("priority", ["high", "normal", "low"]), byAssignee });
}
