import type { Ctx, Task } from "./types";
import { json, missing, taskReply, visibleProject, visibleTask, validation } from "./util";
import { updateTask } from "./task-write";

export async function readTaskRoute(ctx: Ctx): Promise<Response | null> {
  const path = ctx.url.pathname;
  const m = path.match(/^\/tasks\/([^/]+)(?:\/(history|comments|dependencies|transitions)(?:\/([^/]+))?)?$/);
  if (!m) return null;
  const t = visibleTask(ctx, m[1]!);
  if (!t) return missing();
  const tail = m[2], method = ctx.request.method;
  if (!tail && method === "GET") return taskReply(t);
  if (tail === "history" && method === "GET") return json(ctx.store.history.get(t.key) ?? []);
  if (tail === "transitions" && method === "POST") return updateTask(ctx, t, true);
  if (!tail && method === "PATCH") return updateTask(ctx, t);
  if (!tail && method === "DELETE") {
    const project = ctx.store.projects.find(p => p.id === t.projectId)!;
    const role = project.members.get(ctx.user!.id)?.role;
    if (role !== "owner" && t.createdBy !== ctx.user!.id) {
      const { denied } = await import("./util");
      return denied();
    }
    t.deleted = true;
    for (const other of ctx.store.tasks) other.dependencies.delete(t.key);
    return new Response(null, { status: 204 });
  }
  if (tail === "comments") return comments(ctx, t);
  if (tail === "dependencies") return dependencies(ctx, t, m[3]);
  return null;
}

async function comments(ctx: Ctx, t: Task) {
  const list = ctx.store.comments.get(t.key) ?? [];
  if (ctx.request.method === "GET") return json(list);
  if (ctx.request.method !== "POST") return null;
  const b: any = await ctx.request.json().catch(() => null);
  if (!b || typeof b.body !== "string" || !b.body.trim() || b.body.length > 2000)
    return validation("body");
  const item = { id: list.length + 1, authorId: ctx.user!.id, body: b.body.trim(),
    createdAt: new Date(ctx.store.now()).toISOString() };
  list.push(item);
  ctx.store.comments.set(t.key, list);
  return json(item, 201);
}

async function dependencies(ctx: Ctx, t: Task, target?: string): Promise<Response | null> {
  const all = ctx.store.tasks.filter(x => !x.deleted);
  const related = () => ({
    key: t.key,
    dependsOn: [...t.dependencies].sort((a, b) => (all.find(x => x.key === a)?.number ?? 0) -
      (all.find(x => x.key === b)?.number ?? 0)),
    blocks: all.filter(x => x.dependencies.has(t.key)).sort((a, b) => a.number - b.number).map(x => x.key),
  });
  if (ctx.request.method === "GET") return json(related());
  if (ctx.request.method === "DELETE") {
    if (!target || !t.dependencies.delete(target)) return missing();
    return new Response(null, { status: 204 });
  }
  if (ctx.request.method !== "POST") return null;
  const { canWrite, denied, conflict, fail } = await import("./util");
  if (!canWrite(ctx, t)) return denied();
  const b: any = await ctx.request.json().catch(() => null);
  const other = typeof b?.dependsOn === "string" ? all.find(x => x.key === b.dependsOn) : undefined;
  const p = ctx.store.projects.find(x => x.id === t.projectId)!;
  if (!other || other.projectId !== t.projectId || other.key === t.key) return validation("dependsOn");
  if (t.dependencies.has(other.key)) return conflict();
  const reaches = (from: Task, target: string, seen = new Set<string>()): boolean => {
    if (from.key === target) return true;
    if (seen.has(from.key)) return false;
    seen.add(from.key);
    return [...from.dependencies].some(k => {
      const x = all.find(a => a.key === k);
      return x ? reaches(x, target, seen) : false;
    });
  };
  if (reaches(other, t.key)) return fail("dependency_cycle", "Dependency cycle", 409);
  void p;
  t.dependencies.add(other.key);
  return json(related(), 201);
}
