// @ts-nocheck
import type { Project, Task } from "./model";
import { bodyOf, fail, response, validate } from "./http";
import { type Ctx, user, editor } from "./task-helpers";
export async function dependencies(c: Ctx, t: Task, parts: string[], method: string,
  p: Project): Promise<Response> {
  const { s } = c, u = user(c);
  if (parts.length === 3 && method === "GET") {
    const ids = [...t.deps];
    return response({ key: t.key, dependsOn: ids.map(id => s.tasks.find(x => x.id === id)!)
      .filter(Boolean).sort((a, b) => a.number - b.number).map(x => x.key),
      blocks: s.tasks.filter(x => x.deps.has(t.id)).sort((a, b) => a.number - b.number).map(x => x.key) });
  }
  if (parts.length === 3 && method === "POST") {
    if (!editor(p, u.id)) fail(403, "forbidden", "Read only");
    const b = await bodyOf(c.req);
    validate(typeof b.dependsOn === "string", "dependsOn");
    const target = s.tasks.find(x => x.key === b.dependsOn && x.projectId === p.id);
    validate(!!target && target.id !== t.id, "dependsOn");
    if (t.deps.has(target!.id)) fail(409, "conflict", "Dependency already exists");
    const reaches = (from: Task, goal: number, seen = new Set<number>()): boolean => {
      if (from.id === goal) return true;
      if (seen.has(from.id)) return false;
      seen.add(from.id);
      return [...from.deps].some(id => {
        const next = s.tasks.find(x => x.id === id);
        return !!next && reaches(next, goal, seen);
      });
    };
    if (reaches(target!, t.id)) fail(409, "dependency_cycle", "Dependency cycle");
    t.deps.add(target!.id);
    return response({ key: t.key, dependsOn: [...t.deps].map(id => s.tasks.find(x => x.id === id)!)
      .sort((a, b) => a.number - b.number).map(x => x.key), blocks: [] }, 201);
  }
  if (parts.length === 4 && method === "DELETE") {
    if (!editor(p, u.id)) fail(403, "forbidden", "Read only");
    const target = s.tasks.find(x => x.key === parts[3] && x.projectId === p.id);
    if (!target || !t.deps.has(target.id)) fail(404, "not_found", "Not found");
    t.deps.delete(target.id); return response(null, 204);
  }
  fail(404, "not_found", "Not found");
}
