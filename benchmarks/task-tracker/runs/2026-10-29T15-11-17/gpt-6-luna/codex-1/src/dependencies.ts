import {
  body,
  fail,
  json,
  requireRole,
  validate,
  type State,
  type Task,
} from "./model";
export function dependency(
  req: Request,
  t: Task,
  tail: string | undefined,
  s: State,
  u: number,
) {
  if (req.method === "GET")
    return json({
      key: t.key,
      dependsOn: [...t.deps].sort(byTask(s)),
      blocks: s.tasks
        .filter((x) => x.deps.has(t.key))
        .map((x) => x.key)
        .sort(byTask(s)),
    });
  const p = s.projects.find((x) => x.id === t.projectId)!;
  requireRole(p, u);
  if (req.method === "POST")
    return (async () => {
      const b = await body(req),
        dep = s.tasks.find((x) => x.key === b.dependsOn);
      validate(
        !!dep && dep.projectId === t.projectId && dep !== t,
        "dependsOn",
      );
      if (t.deps.has(dep!.key)) fail(409, "conflict");
      const reaches = (
        from: Task,
        target: string,
        seen = new Set<string>(),
      ): boolean => {
        if (from.key === target) return true;
        if (seen.has(from.key)) return false;
        seen.add(from.key);
        return [...from.deps].some((k) => {
          const x = s.tasks.find((v) => v.key === k);
          return !!x && reaches(x, target, seen);
        });
      };
      if (reaches(dep!, t.key)) fail(409, "dependency_cycle");
      t.deps.add(dep!.key);
      return json(
        { key: t.key, dependsOn: [...t.deps].sort(byTask(s)), blocks: [] },
        201,
      );
    })();
  if (req.method === "DELETE" && tail) {
    if (!t.deps.has(tail)) fail(404, "not_found");
    t.deps.delete(tail);
    return new Response(null, { status: 204 });
  }
  return undefined;
}
function byTask(s: State) {
  return (a: string, b: string) => {
    const x = s.tasks.find((t) => t.key === a),
      y = s.tasks.find((t) => t.key === b);
    return (x?.number || 0) - (y?.number || 0);
  };
}
