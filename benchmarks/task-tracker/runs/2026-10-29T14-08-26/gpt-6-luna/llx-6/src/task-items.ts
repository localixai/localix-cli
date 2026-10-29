import { check, fail, response } from "./http";
import { memberRole, type State, type Task } from "./model";
import { historyChange, memberCanRead, taskFor } from "./tasks";

export function taskItemRoutes(s: State, user: number | undefined, path: string, method: string,
  body: any, now: string, headers: Headers) {
  const match = /^\/tasks\/([A-Z]+-\d+)(?:\/(transitions|history|comments|dependencies))?(?:\/(comments|dependencies)\/([^/]+))?$/.exec(path);
  if (!match) return;
  const task = taskFor(s, match[1]!), project = s.projects.find(p => p.id === task.projectId)!;
  if (!user || !memberCanRead(s, project, user)) fail(user ? 404 : 401, user ? "not_found" : "unauthorized");
  const role = memberRole(s, project, user!), writable = role === "owner" || role === "member";
  const version = headers.get("If-Match");
  const checkVersion = () => {
    if (version && version !== `"${task.version}"`) fail(412, "precondition_failed", "version changed");
  };
  const answer = (status = 200) => response(task, status, { ETag: `"${task.version}"` });
  const resource = match[2], child = match[4];
  if (!resource && !child && method === "GET") return answer();
  if (resource === "history" && method === "GET") return response(s.history.get(String(task.id)) ?? []);
  if (resource === "transitions" && method === "POST") {
    if (!writable) fail(403, "forbidden");
    checkVersion();
    check(["open", "in_progress", "done"].includes(body.to), "to");
    const allowed: Record<string, string[]> = { open: ["in_progress"], in_progress: ["open", "done"], done: ["open"] };
    if (!allowed[task.status]!.includes(body.to)) fail(409, "invalid_transition");
    if (body.to === "done" && [...(s.deps.get(task.id) ?? [])].some(id =>
      s.tasks.find(t => t.id === id)?.status !== "done")) fail(409, "blocked");
    const old = task.status;
    task.status = body.to; task.updatedAt = now; task.version++;
    historyChange(s, task, user!, "status", old, task.status, now);
    return answer();
  }
  if (resource === "comments" && method === "POST") {
    check(typeof body.body === "string" && body.body.trim().length > 0 &&
      body.body.length <= 2000, "body");
    const key = String(task.id), list = s.comments.get(key) ?? [];
    const comment = { id: list.length + 1, authorId: user, body: body.body, createdAt: now };
    list.push(comment); s.comments.set(key, list); return response(comment, 201);
  }
  if (resource === "comments" && method === "GET") return response(s.comments.get(String(task.id)) ?? []);
  if (resource === "dependencies" && method === "GET") return response(dependencyView(s, task));
  if (resource === "dependencies" && method === "POST") {
    if (!writable) fail(403, "forbidden");
    const key = body.dependsOn;
    const other = typeof key === "string" ? s.tasks.find(t => t.key === key) : undefined;
    if (!other || other.projectId !== task.projectId || other.id === task.id)
      fail(400, "validation", "invalid dependency", "dependsOn");
    const set = s.deps.get(task.id) ?? new Set<number>();
    if (set.has(other!.id)) fail(409, "conflict");
    if (reaches(s, other!.id, task.id)) fail(409, "dependency_cycle");
    set.add(other!.id); s.deps.set(task.id, set);
    return response(dependencyView(s, task), 201);
  }
  if (resource === "dependencies" && child && method === "DELETE") {
    const other = s.tasks.find(t => t.key === child);
    const set = s.deps.get(task.id);
    if (!other || !set?.delete(other.id)) fail(404, "not_found");
    return response(null, 204);
  }
  if (!resource && !child && method === "PATCH") {
    if (!writable) fail(403, "forbidden");
    checkVersion();
    const fields = ["assigneeId", "description", "labels", "priority", "title"];
    for (const key of Object.keys(body)) if (!fields.includes(key)) fail(400, "validation", "unknown field", key);
    const fake = { ...body, title: body.title ?? task.title };
    const cleaned = awaitClean(fake, user!, project, s);
    let changed = false;
    for (const key of fields) {
      if (!(key in body)) continue;
      const value = key === "assigneeId" ? body[key] : (cleaned as any)[key];
      const old = (task as any)[key];
      if (JSON.stringify(old) !== JSON.stringify(value)) {
        historyChange(s, task, user!, key!, old, value, now);
        (task as any)[key] = value; changed = true;
      }
    }
    if (changed) { task.version++; task.updatedAt = now; }
    return answer();
  }
  if (!resource && !child && method === "DELETE") {
    if (!writable) fail(403, "forbidden");
    if (role !== "owner" && task.createdBy !== user) fail(403, "forbidden");
    s.tasks.splice(s.tasks.indexOf(task), 1);
    for (const [id, deps] of s.deps) { deps.delete(task.id); if (id === task.id) s.deps.delete(id); }
    s.comments.delete(String(task.id)); s.history.delete(String(task.id));
    return response(null, 204);
  }
}
import { cleanTask } from "./tasks";
function awaitClean(body: any, user: number, project: any, state: State) {
  return cleanTask(body, user, project, state);
}
function reaches(s: State, from: number, goal: number, seen = new Set<number>()): boolean {
  if (from === goal) return true;
  if (seen.has(from)) return false;
  seen.add(from);
  return [...(s.deps.get(from) ?? [])].some(id => reaches(s, id, goal, seen));
}
function dependencyView(s: State, task: Task) {
  const key = (id: number) => s.tasks.find(t => t.id === id)?.key;
    const dependsOn = [...(s.deps.get(task.id) ?? [])].map(key).filter(Boolean).sort((a,b)=>
      Number(a!.split("-")[1])-Number(b!.split("-")[1]));
  const blocks = [...s.deps].filter(([, deps]) => deps.has(task.id)).map(([id]) => key(id))
    .filter(Boolean).sort();
  return { key: task.key, dependsOn, blocks };
}
