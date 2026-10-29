import type { State, Task } from "./model";
import { taskByKey } from "./model";
import { ApiError, body, respond, writer } from "./http";
function ordered(s: State, ids: Set<number>) {
  return [...ids]
    .map((id) => s.tasks.find((t) => t.id === id))
    .filter((t): t is Task => !!t)
    .sort((a, b) => a.number - b.number)
    .map((t) => t.key);
}
export async function dependencies(
  req: Request,
  t: Task,
  tail: string | undefined,
  s: State,
  uid: number,
): Promise<Response | null> {
  const method = req.method;
  if (method === "GET") {
    const blocks = new Set(
      s.tasks.filter((x) => x.dependsOn.has(t.id)).map((x) => x.id),
    );
    return respond({
      key: t.key,
      dependsOn: ordered(s, t.dependsOn),
      blocks: ordered(s, blocks),
    });
  }
  if (method === "POST") {
    writer(s, t, uid);
    const b = await body(req),
      dep = taskByKey(s, String(b.dependsOn));
    if (!dep || dep.id === t.id || dep.projectId !== t.projectId)
      throw new ApiError(
        400,
        "validation",
        "dependsOn is invalid",
        "dependsOn",
      );
    if (t.dependsOn.has(dep.id))
      throw new ApiError(409, "conflict", "dependency already exists");
    const reaches = (
      from: number,
      target: number,
      seen = new Set<number>(),
    ): boolean => {
      if (from === target) return true;
      if (seen.has(from)) return false;
      seen.add(from);
      const node = s.tasks.find((x) => x.id === from);
      return (
        !!node && [...node.dependsOn].some((x) => reaches(x, target, seen))
      );
    };
    if (reaches(dep.id, t.id))
      throw new ApiError(409, "dependency_cycle", "dependency creates a cycle");
    t.dependsOn.add(dep.id);
    return respond(
      {
        key: t.key,
        dependsOn: ordered(s, t.dependsOn),
        blocks: ordered(
          s,
          new Set(
            s.tasks.filter((x) => x.dependsOn.has(t.id)).map((x) => x.id),
          ),
        ),
      },
      201,
    );
  }
  if (method === "DELETE" && tail) {
    writer(s, t, uid);
    const dep = taskByKey(s, decodeURIComponent(tail));
    if (!dep || !t.dependsOn.delete(dep.id))
      throw new ApiError(404, "not_found", "dependency not found");
    return respond(null, 204);
  }
  return null;
}
