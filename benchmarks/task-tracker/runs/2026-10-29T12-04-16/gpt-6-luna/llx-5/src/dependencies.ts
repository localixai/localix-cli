import { member, taskByKey, validate, type Ctx, type Task } from "./model";
import { json } from "./response";

export function dependencyRoute(c: Ctx, task: Task, key?: string) {
  const deps = c.s.deps.get(task.id) || new Set<number>();
  if (!key && c.req.method === "GET") {
    const dependsOn = [...deps].map(id => c.s.tasks.find(t => t.id === id)?.key)
      .filter((value): value is string => Boolean(value)).sort(sortKeys);
    const blocks = [...c.s.deps].filter(([, set]) => set.has(task.id))
      .map(([id]) => c.s.tasks.find(t => t.id === id)?.key)
      .filter((value): value is string => Boolean(value)).sort(sortKeys);
    return json({ key: task.key, dependsOn, blocks });
  }
  if (!key && c.req.method === "POST") {
    const project = c.s.projects.find(p => p.id === task.projectId)!;
    if (member(c.s, project, c.user!) === "viewer")
      throw Object.assign(Error("Forbidden"), { status: 403, code: "forbidden" });
    const other = taskByKey(c.s, c.body.dependsOn);
    validate(other && other.projectId === task.projectId && other.id !== task.id, "dependsOn");
    if (deps.has(other!.id))
      throw Object.assign(Error("Already dependent"), { status: 409, code: "conflict" });
    if (reaches(c, other!.id, task.id))
      throw Object.assign(Error("Dependency cycle"), { status: 409, code: "dependency_cycle" });
    deps.add(other!.id);
    c.s.deps.set(task.id, deps);
    return json({
      key: task.key,
      dependsOn: [...deps].map(id => c.s.tasks.find(t => t.id === id)!.key).sort(sortKeys),
      blocks: [...c.s.deps].filter(([, set]) => set.has(task.id))
        .map(([id]) => c.s.tasks.find(t => t.id === id)!.key).sort(sortKeys),
    }, 201);
  }
  if (key && c.req.method === "DELETE") {
    const other = taskByKey(c.s, key);
    if (!other || !deps.delete(other.id))
      throw Object.assign(Error("Not found"), { status: 404, code: "not_found" });
    return new Response(null, { status: 204 });
  }
}

function reaches(c: Ctx, from: number, target: number, seen = new Set<number>()): boolean {
  if (from === target) return true;
  if (seen.has(from)) return false;
  seen.add(from);
  return [...(c.s.deps.get(from) || [])].some(next => reaches(c, next, target, seen));
}
function sortKeys(a: string, b: string) {
  return Number(a.split("-").pop()) - Number(b.split("-").pop());
}
