import type { State, Task } from "./types";
import { fail, valid, projectFor, taskFor, writer, makeTask, taskJson } from "./util";
const json = (x: unknown, status = 200, headers: Record<string, string> = {}) =>
  Response.json(x, { status, headers });
const noContent = () => new Response(null, { status: 204 });
export function tasks(s: State, method: string, path: string, b: any, uid: number,
  headers: Headers, url: URL): Response {
  const m = path.match(/^\/tasks\/([^/]+)(.*)$/);
  if (!m) return fail(404, "not_found");
  const key = m[1]!, sub = m[2] ?? "", t = taskFor(s, key, uid);
  const p = s.projects.find((x) => x.id === t.projectId)!;
  if (!sub && method === "GET") return json(taskJson(t), 200, { ETag: `"${t.version}"` });
  if (["PATCH", "POST"].includes(method) && ["", "/transitions"].includes(sub)) {
    writer(p, uid);
    const match = headers.get("If-Match");
    if (match && match !== `"${t.version}"`) fail(412, "precondition_failed");
  }
  if (!sub && method === "PATCH") return patch(s, t, p, uid, b);
  if (!sub && method === "DELETE") {
    if (uid !== p.ownerId && t.createdBy !== uid) fail(403, "forbidden");
    s.tasks = s.tasks.filter((x) => x !== t);
    for (const x of s.tasks) x.dependsOn.delete(key);
    return noContent();
  }
  if (sub === "/transitions" && method === "POST") return transition(s, t, uid, b);
  if (sub === "/history" && method === "GET") return json(t.history);
  if (sub === "/comments" && method === "GET") return json(t.comments);
  if (sub === "/comments" && method === "POST") {
    const body = typeof b.body === "string" ? b.body.trim() : "";
    valid(body.length > 0 && body.length <= 2000, "body");
    const c = { id: ++s.ids.comment, authorId: uid, body, createdAt: new Date(s.now()).toISOString() };
    t.comments.push(c); return json(c, 201);
  }
  if (sub === "/dependencies" && method === "GET") return json(dependencyLists(s, t));
  if (sub === "/dependencies" && method === "POST") return addDependency(s, t, p, uid, b);
  const dm = sub.match(/^\/dependencies\/(.+)$/);
  if (dm && method === "DELETE") {
    if (!t.dependsOn.delete(dm[1]!)) fail(404, "not_found");
    return noContent();
  }
  return fail(404, "not_found");
}
function patch(s: State, t: Task, p: State["projects"][number], uid: number, b: any): Response {
  const names = Object.keys(b), allowed = ["title", "description", "priority", "labels", "assigneeId"];
  const unknown = names.find((x) => !allowed.includes(x));
  if (unknown) fail(400, "validation", `Invalid ${unknown}`, unknown);
  const candidate = { title: t.title, description: t.description, priority: t.priority,
    labels: t.labels, assigneeId: t.assigneeId, ...b };
  const temp = makeTaskCheck(candidate, p);
  const changes: Record<string, unknown> = {};
  for (const name of names.sort()) {
    const next = (temp as any)[name], prev = (t as any)[name];
    if (JSON.stringify(next) !== JSON.stringify(prev)) {
      changes[name] = next;
      t.history.push({ at: new Date(s.now()).toISOString(), by: uid, field: name, from: prev, to: next });
    }
  }
  Object.assign(t, changes);
  if (Object.keys(changes).length) { t.version++; t.updatedAt = new Date(s.now()).toISOString(); }
  return json(taskJson(t), 200, { ETag: `"${t.version}"` });
}
function makeTaskCheck(b: any, p: State["projects"][number]): any {
  const labels = b.labels ?? [];
  valid(typeof b.title === "string" && b.title.trim().length > 0 && b.title.trim().length <= 200, "title");
  valid(typeof b.description === "string", "description");
  valid(["low", "normal", "high"].includes(b.priority), "priority");
  valid(Array.isArray(labels) && labels.every((x: unknown) =>
    typeof x === "string" && x.trim().length > 0 && x.trim().length <= 30 && !/\s/.test(x.trim())), "labels");
  valid(b.assigneeId === null || (Number.isInteger(b.assigneeId) &&
    (b.assigneeId === p.ownerId || p.members.has(b.assigneeId))), "assigneeId");
  return { ...b, title: b.title.trim(), labels: [...new Set(labels.map((x: string) =>
    x.trim().toLowerCase()))].sort() };
}
function transition(s: State, t: Task, uid: number, b: any): Response {
  valid(["open", "in_progress", "done"].includes(b.to), "to");
  const allowed: Record<string, string[]> = { open: ["in_progress"], in_progress: ["open", "done"], done: ["open"] };
  if (!allowed[t.status]?.includes(b.to)) fail(409, "invalid_transition");
  if (b.to === "done" && [...t.dependsOn].some((key) => s.tasks.some((x) => x.key === key && x.status !== "done")))
    fail(409, "blocked");
  const old = t.status; t.status = b.to; t.version++; t.updatedAt = new Date(s.now()).toISOString();
  t.history.push({ at: t.updatedAt, by: uid, field: "status", from: old, to: t.status });
  return json(taskJson(t), 200, { ETag: `"${t.version}"` });
}
function dependencyLists(s: State, t: Task) {
  const order = (a: string, b: string) => Number(a.split("-").pop()) - Number(b.split("-").pop());
  return { key: t.key, dependsOn: [...t.dependsOn].sort(order),
    blocks: s.tasks.filter((x) => x.dependsOn.has(t.key)).map((x) => x.key).sort(order) };
}
function addDependency(s: State, t: Task, p: State["projects"][number], uid: number, b: any): Response {
  writer(p, uid);
  const dep = typeof b.dependsOn === "string" ? s.tasks.find((x) => x.key === b.dependsOn) : undefined;
  valid(!!dep && dep.key !== t.key && dep.projectId === t.projectId, "dependsOn");
  if (t.dependsOn.has(dep!.key)) fail(409, "conflict");
  const reaches = (key: string, seen = new Set<string>()): boolean => {
    if (key === t.key) return true;
    if (seen.has(key)) return false;
    seen.add(key);
    return [...(s.tasks.find((x) => x.key === key)?.dependsOn ?? [])].some((x) => reaches(x, seen));
  };
  if (reaches(dep!.key)) fail(409, "dependency_cycle");
  t.dependsOn.add(dep!.key);
  return json(dependencyLists(s, t), 201);
}
