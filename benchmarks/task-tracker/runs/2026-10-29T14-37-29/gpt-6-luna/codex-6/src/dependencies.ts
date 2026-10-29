import type { Ctx, Result, Task } from "./types";
import { fail, member, ok } from "./util";
export function dependencyRoute(
  c: Ctx,
  t: Task,
  m: string,
  tail?: string,
): Result {
  const deps = c.state.deps.get(t.id)!;
  const keys = (ids: Set<number>) =>
    [...ids]
      .map((id) => c.state.tasks.find((x) => x.id === id))
      .filter(Boolean)
      .sort((a, b) => a!.number - b!.number)
      .map((x) => x!.key);
  if (m === "GET")
    return ok({
      key: t.key,
      dependsOn: keys(deps),
      blocks: keys(
        new Set(
          [...c.state.deps].filter(([, d]) => d.has(t.id)).map(([id]) => id),
        ),
      ),
    });
  if (member(c, t.projectId) === "viewer") return fail(403, "forbidden");
  if (m === "POST") {
    const key = c.body?.dependsOn,
      other = c.state.tasks.find((x) => x.key === key);
    if (
      typeof key !== "string" ||
      key === t.key ||
      !other ||
      other.projectId !== t.projectId
    )
      return fail(400, "validation", "invalid dependency", "dependsOn");
    if (deps.has(other.id)) return fail(409, "conflict");
    if (reaches(c, other.id, t.id, new Set()))
      return fail(409, "dependency_cycle");
    deps.add(other.id);
    return ok({ key: t.key, dependsOn: keys(deps), blocks: [] }, 201);
  }
  if (m === "DELETE" && tail) {
    const other = c.state.tasks.find((x) => x.key === tail);
    if (!other || !deps.has(other.id)) return fail(404, "not_found");
    deps.delete(other.id);
    return ok(undefined, 204);
  }
  return fail(404, "not_found");
}
function reaches(c: Ctx, from: number, to: number, seen: Set<number>): boolean {
  if (from === to) return true;
  if (seen.has(from)) return false;
  seen.add(from);
  for (const d of c.state.deps.get(from) || [])
    if (reaches(c, d, to, seen)) return true;
  return false;
}
