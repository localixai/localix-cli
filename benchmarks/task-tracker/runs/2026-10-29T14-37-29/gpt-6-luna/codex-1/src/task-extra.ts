import { bodyOf, fail, invalid, response } from "./errors";
import { member, type Context } from "./model";
import { requireUser } from "./auth";

function find(ctx: Context, key: string) {
  const task = ctx.state.tasks.find((t) => t.key === key);
  const p = task && ctx.state.projects.find((x) => x.id === task.projectId);
  if (!task || !p) fail(404, "not_found", "Task not found");
  return { task, project: p };
}
function canRead(ctx: Context, req: Request, key: string) {
  const user = requireUser(ctx, req);
  const found = find(ctx, key);
  const role = member(ctx, found.project.id, user.id);
  if (!role) fail(404, "not_found", "Task not found");
  return { ...found, user, role };
}
export async function extraRoute(ctx: Context, req: Request, key: string, sub: string, method: string) {
  if (sub === "dependencies" || sub.startsWith("dependencies/")) {
    return dependencies(ctx, req, key, sub, method);
  }
  if (sub === "comments") return commentRoute(ctx, req, key, method);
  return null;
}
async function dependencies(ctx: Context, req: Request, key: string, sub: string, method: string) {
  const m = key;
  if (method === "GET") {
    const { task } = canRead(ctx, req, m);
    const own = [...(ctx.state.deps.get(task.id) ?? [])]
      .map((id) => ctx.state.tasks.find((t) => t.id === id)?.key)
      .filter((key): key is string => Boolean(key)).sort(keySort);
    const blocks = [...ctx.state.deps.entries()].filter(([, deps]) => deps.has(task.id))
      .map(([id]) => ctx.state.tasks.find((t) => t.id === id)?.key)
      .filter((key): key is string => Boolean(key)).sort(keySort);
    return response({ key: task.key, dependsOn: own, blocks });
  }
  const { task, project, role } = canRead(ctx, req, m);
  if (role === "viewer") fail(403, "forbidden", "Write access required");
  const depKey = sub.split("/")[1];
  if (method === "DELETE") {
    const dep = ctx.state.tasks.find((t) => t.key === depKey && t.projectId === project.id);
    if (!dep || !ctx.state.deps.get(task.id)?.has(dep.id)) fail(404, "not_found", "Dependency not found");
    ctx.state.deps.get(task.id)!.delete(dep.id);
    return response(null, 204);
  }
  if (method !== "POST") return null;
  const b = await bodyOf(req);
  if (typeof b.dependsOn !== "string") invalid("dependsOn");
  const other = ctx.state.tasks.find((t) => t.key === b.dependsOn && t.projectId === project.id);
  if (!other || other.id === task.id) invalid("dependsOn");
  const deps = ctx.state.deps.get(task.id)!;
  if (deps.has(other.id)) fail(409, "conflict", "Dependency already exists");
  if (reaches(ctx, other.id, task.id, new Set())) fail(409, "dependency_cycle", "Dependency creates a cycle");
  deps.add(other.id);
  const keys = [...deps].map((id) => ctx.state.tasks.find((t) => t.id === id)!.key)
    .sort(keySort);
  return response({ key: task.key, dependsOn: keys, blocks: [] }, 201);
}
function reaches(ctx: Context, from: number, target: number, seen: Set<number>): boolean {
  if (from === target) return true;
  if (seen.has(from)) return false;
  seen.add(from);
  return [...(ctx.state.deps.get(from) ?? [])].some((id) => reaches(ctx, id, target, seen));
}
function keySort(a: string, b: string) {
  const x = Number(a.split("-").at(-1));
  const y = Number(b.split("-").at(-1));
  return x - y;
}
async function commentRoute(ctx: Context, req: Request, key: string, method: string) {
  const { task, user } = canRead(ctx, req, key);
  const comments = ctx.state.comments.get(task.id) ?? [];
  if (method === "GET") return response(comments);
  if (method !== "POST") return null;
  const b = await bodyOf(req);
  if (typeof b.body !== "string" || !b.body.trim() || b.body.length > 2000) invalid("body");
  const comment = { id: ctx.state.nextComment++, authorId: user.id, body: b.body,
    createdAt: new Date(ctx.now()).toISOString() };
  comments.push(comment);
  ctx.state.comments.set(task.id, comments);
  return response(comment, 201);
}
