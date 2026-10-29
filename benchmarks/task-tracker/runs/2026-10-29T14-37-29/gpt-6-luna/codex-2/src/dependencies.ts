import { Context, Task, fail, json, memberRole, taskByKey } from "./model";

export function dependencyRoutes(
  c: Context,
  t: Task,
  parts: string[],
  method: string,
  b: any,
) {
  const sets = c.state.deps,
    own = sets.get(t.id)!;
  if (parts.length === 4 && method === "GET") {
    const deps = [...own]
      .map((id) => c.state.tasks.find((x) => x.id === id)!)
      .filter(Boolean)
      .sort((a, b) => a.number - b.number)
      .map((x) => x.key);
    const blocks = [...sets]
      .filter(([, d]) => d.has(t.id))
      .map(([id]) => c.state.tasks.find((x) => x.id === id)!)
      .filter(Boolean)
      .sort((a, b) => a.number - b.number)
      .map((x) => x.key);
    return json({ key: t.key, dependsOn: deps, blocks });
  }
  if (parts.length === 4 && method === "POST") {
    if (memberRole(c.state, t.projectId, c.userId!) === "viewer")
      return fail("forbidden", "write access required", 403);
    const other = taskByKey(c.state, b.dependsOn);
    if (!other || other.id === t.id || other.projectId !== t.projectId)
      return fail("validation", "invalid dependency", 400, "dependsOn");
    if (own.has(other.id)) return fail("conflict", "dependency exists", 409);
    const reaches = (id: number, seen = new Set<number>()): boolean =>
      id === t.id
        ? true
        : seen.has(id)
          ? false
          : (seen.add(id),
            [...(sets.get(id) ?? [])].some((x) => reaches(x, seen)));
    if (reaches(other.id))
      return fail("dependency_cycle", "dependency cycle", 409);
    own.add(other.id);
    return json(
      {
        key: t.key,
        dependsOn: [...own]
          .map((i) => c.state.tasks.find((x) => x.id === i)!.key)
          .sort((a, b) => Number(a.split("-")[1]) - Number(b.split("-")[1])),
        blocks: [],
      },
      201,
    );
  }
  if (parts.length === 5 && method === "DELETE") {
    const other = taskByKey(c.state, parts[4]!);
    if (!other || !own.has(other.id))
      return fail("not_found", "not found", 404);
    own.delete(other.id);
    return { status: 204 };
  }
}
