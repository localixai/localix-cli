import { empty, fail, ok, read, validation } from "./http";
import type { Context, Project, Task, User } from "./model";
import { addMember, memberRole, projectView, removeMember, writable } from "./projects";
import { byKey, checkVersion, plainTask, transition, update } from "./taskops";
import { addDependency, bulk, comment, deleteTask, dependencies, list, stats } from "./tasks-extra";
import { createWithIdempotency } from "./idempotency";
export async function projectRoute(c: Context, req: Request, url: URL, p: Project,
  user: User, params: Record<string, string>, method: string) {
  const path = url.pathname;
  if (path === `/projects/${p.id}`) return projectView(p);
  if (path.endsWith("/members")) {
    if (method === "GET") return [...c.s.members.get(p.id)!].sort((a, b) => a[0] - b[0])
      .map(([userId, role]) => ({ userId, role }));
    if (p.ownerId !== user.id) fail(403, "forbidden", "Only the owner can add members");
    return addMember(c, p, await read(req));
  }
  if (params.uid) {
    writable(c, p, user.id);
    const uid = /^\d+$/.test(params.uid) ? Number(params.uid) : -1;
    removeMember(c, p, uid); return null;
  }
  if (path.endsWith("/stats")) return stats(c, p);
  if (path.endsWith("/tasks/bulk")) {
    writable(c, p, user.id); return bulk(c, p, user, await read(req));
  }
  if (path.endsWith("/tasks")) {
    if (method === "GET") {
      const result = list(c, p, url, user.id); return result;
    }
    writable(c, p, user.id);
    const body = await read(req);
    return createWithIdempotency(c, p, user, body, req.headers.get("Idempotency-Key"));
  }
  fail(404, "not_found", "Route not found");
}
export async function taskRoute(c: Context, req: Request, t: Task, p: Project,
  user: User, params: Record<string, string>, method: string) {
  const path = new URL(req.url).pathname;
  if (path === `/tasks/${t.key}`) {
    if (method === "GET") return { result: plainTask(t), headers: { ETag: `"${t.version}"` } };
    writable(c, p, user.id);
    if (method === "DELETE") {
      if (memberRole(c, p, user.id) !== "owner" && t.createdBy !== user.id)
        fail(403, "forbidden", "Cannot delete another member's task");
      deleteTask(c, t); return { status: 204, result: null };
    }
    checkVersion(req, t); const changed = update(c, t, user, await read(req));
    return { result: plainTask(changed), headers: { ETag: `"${changed.version}"` } };
  }
  if (path.endsWith("/comments")) {
    if (method === "GET") return { result: c.s.comments.get(t.id) };
    return { result: comment(c, t, user, await read(req)), status: 201 };
  }
  if (path.endsWith("/history")) return { result: c.s.history.get(t.id) };
  if (path.endsWith("/transitions")) {
    writable(c, p, user.id); checkVersion(req, t);
    const body = await read(req); transition(c, t, user, body.to);
    return { result: plainTask(t), headers: { ETag: `"${t.version}"` } };
  }
  if (path.endsWith("/dependencies")) {
    if (method === "GET") return { result: dependencies(c, t) };
    writable(c, p, user.id); return { result: addDependency(c, t, await read(req)), status: 201 };
  }
  if (params.dep) {
    writable(c, p, user.id);
    const { t: dep } = byKey(c, params.dep, user.id)!;
    const set = c.s.deps.get(t.id)!;
    if (!set.delete(dep!.id)) fail(404, "not_found", "Dependency not found");
    return { status: 204, result: null };
  }
  fail(404, "not_found", "Route not found");
}
