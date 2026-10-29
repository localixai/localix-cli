import {
  body,
  err,
  json,
  role,
  taskFor,
  validation,
  type Ctx,
  type Task,
} from "./model";
export async function dependencies(
  c: Ctx,
  t: Task,
  path: string,
  method: string,
) {
  const del = path.match(/\/dependencies\/([^/]+)$/);
  if (del && method === "DELETE") {
    if (
      role(
        c,
        c.db.projects.find((p) => p.id === t.projectId)!,
      ) === "viewer"
    )
      return err("forbidden", 403);
    const d = taskFor(c, del[1]!);
    if (!d || !t.depends.includes(d.id)) return err("not_found", 404);
    t.depends = t.depends.filter((x) => x !== d.id);
    return new Response(null, { status: 204 });
  }
  if (method === "GET") {
    const deps = t.depends
      .map((id) => c.db.tasks.find((x) => x.id === id && !x.deleted))
      .filter(Boolean) as Task[];
    const blocks = c.db.tasks.filter(
      (x) => !x.deleted && x.depends.includes(t.id),
    );
    return json({
      key: t.key,
      dependsOn: deps.sort((a, b) => a.number - b.number).map((x) => x.key),
      blocks: blocks.sort((a, b) => a.number - b.number).map((x) => x.key),
    });
  }
  if (method === "POST") {
    if (
      role(
        c,
        c.db.projects.find((p) => p.id === t.projectId)!,
      ) === "viewer"
    )
      return err("forbidden", 403);
    const b = await body(c);
    if (!b) return validation();
    const d = typeof b.dependsOn === "string" ? taskFor(c, b.dependsOn) : null;
    if (!d || d.id === t.id || d.projectId !== t.projectId)
      return validation("dependsOn");
    if (t.depends.includes(d.id)) return err("conflict", 409);
    const reaches = (id: number, seen = new Set<number>()): boolean => {
      if (id === t.id) return true;
      if (seen.has(id)) return false;
      seen.add(id);
      return (c.db.tasks.find((x) => x.id === id)?.depends || []).some((n) =>
        reaches(n, seen),
      );
    };
    if (reaches(d.id)) return err("dependency_cycle", 409);
    t.depends.push(d.id);
    return json(
      {
        key: t.key,
        dependsOn: t.depends
          .map((id) => c.db.tasks.find((x) => x.id === id)!)
          .sort((a, b) => a.number - b.number)
          .map((x) => x.key),
        blocks: [],
      },
      201,
    );
  }
  return null;
}
