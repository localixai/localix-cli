import type { State, Task } from "./model";
import { auth, bad, bodyOf, conflict, fail, forbidden, iso, json } from "./http";
import { member, missing, publicTask, taskFor, visibleRole } from "./http";
import { validateTask } from "./task-create";
function etag(task: Task) { return `"${task.version}"`; }
function checkVersion(request: Request, task: Task) {
  const match = request.headers.get("If-Match");
  if (match && match !== etag(task)) {
    fail(412, "precondition_failed", "Precondition failed");
  }
}
function changed(task: Task, uid: number, field: string, from: unknown, to: unknown, at: string) {
  task.history.push({ at, by: uid, field, from, to });
  (task as any)[field] = to;
}
export async function taskRoute(state: State, request: Request, path: string, method: string) {
  const match = path.match(/^\/tasks\/([A-Z]+-\d+)(?:\/(comments|history|dependencies|transitions)(?:\/([^/]+))?)?$/);
  if (!match) return null;
  const key = match[1]!;
  const section = match[2];
  const extra = match[3];
  const uid = auth(state, request);
  const task = taskFor(state, key, uid);
  const role = visibleRole(state, task.projectId, uid)!;
  if (!section && method === "GET") return json(publicTask(task), 200, { ETag: etag(task) });
  if (!section && method === "DELETE") {
    if (role === "viewer" || (role !== "owner" && task.createdBy !== uid)) forbidden();
    state.tasks = state.tasks.filter((t) => t !== task);
    for (const other of state.tasks) other.dependsOn = other.dependsOn.filter((k) => k !== key);
    return new Response(null, { status: 204 });
  }
  if (section === "history" && method === "GET") return json(task.history);
  if (section === "comments") {
    if (method === "GET") return json(task.comments);
    if (method === "POST") {
      const b = await bodyOf(request);
      if (typeof b.body !== "string" || !b.body.trim() || b.body.length > 2000) bad("body");
      const comment = { id: state.nextComment++, authorId: uid, body: b.body.trim(), createdAt: iso(state) };
      task.comments.push(comment);
      return json(comment, 201);
    }
  }
  if (section === "dependencies") {
    if (method === "GET") {
      const blocks = state.tasks.filter((t) => t.dependsOn.includes(key)).map((t) => t.key).sort(keySort);
      return json({ key, dependsOn: [...task.dependsOn].sort(keySort), blocks });
    }
    if (method === "POST") {
      if (role === "viewer") forbidden();
      const b = await bodyOf(request);
      const other = state.tasks.find((t) => t.key === b.dependsOn);
      if (!other || other.projectId !== task.projectId || other.key === key) return bad("dependsOn");
      if (task.dependsOn.includes(other.key)) conflict();
      if (reachable(state, other.key, key)) conflict("dependency_cycle");
      task.dependsOn.push(other.key);
      return json({ key, dependsOn: task.dependsOn, blocks: [] }, 201);
    }
    if (method === "DELETE" && extra) {
      const index = task.dependsOn.indexOf(extra);
      if (index < 0) return missing();
      task.dependsOn.splice(index, 1);
      return new Response(null, { status: 204 });
    }
  }
  if (section === "transitions" && method === "POST") {
    if (role === "viewer") forbidden();
    checkVersion(request, task);
    const b = await bodyOf(request);
    if (!["open", "in_progress", "done"].includes(b.to)) bad("to");
    const valid = task.status === "open" ? b.to === "in_progress" :
      task.status === "in_progress" ? ["done", "open"].includes(b.to) : b.to === "open";
    if (!valid) conflict("invalid_transition");
    if (b.to === "done" && task.dependsOn.some((k) =>
      state.tasks.some((t) => t.key === k && t.status !== "done"))) conflict("blocked");
    const at = iso(state);
    changed(task, uid, "status", task.status, b.to, at);
    task.version++;
    task.updatedAt = at;
    return json(publicTask(task), 200, { ETag: etag(task) });
  }
  if (!section && method === "PATCH") {
    if (role === "viewer") forbidden();
    checkVersion(request, task);
    const b = await bodyOf(request);
    const allowed = ["title", "description", "priority", "labels", "assigneeId"];
    for (const name of Object.keys(b)) if (!allowed.includes(name)) bad(name);
    const clean = validateTask(state, task.projectId, { ...task, ...b });
    const fields = Object.keys(b).sort();
    const updates = fields.map((field) => [field, (task as any)[field], (clean as any)[field]] as const)
      .filter(([, from, to]) => JSON.stringify(from) !== JSON.stringify(to));
    if (updates.length) {
      const at = iso(state);
      for (const [field, from, to] of updates) changed(task, uid, field, from, to, at);
      task.version++;
      task.updatedAt = at;
    }
    return json(publicTask(task), 200, { ETag: etag(task) });
  }
  return null;
}
function keySort(a: string, b: string) { return Number(a.split("-")[1]) - Number(b.split("-")[1]); }
function reachable(state: State, from: string, target: string): boolean {
  const task = state.tasks.find((t) => t.key === from);
  return !!task && (task.dependsOn.includes(target) || task.dependsOn.some((next) => reachable(state, next, target)));
}
