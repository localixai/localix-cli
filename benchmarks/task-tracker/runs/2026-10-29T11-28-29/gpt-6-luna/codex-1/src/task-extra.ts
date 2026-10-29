import {
  canWrite,
  err,
  fail,
  json,
  role,
  stamp,
  taskByKey,
  type State,
} from "./model";
export function extra(
  s: State,
  method: string,
  path: string,
  uid: number,
  body: any,
) {
  const dep = path.match(/^\/tasks\/([^/]+)\/dependencies(?:\/([^/]+))?$/);
  if (dep) {
    const t = taskByKey(s, dep[1]!);
    if (!t || !member(s, t.projectId, uid))
      return fail(404, "not_found", "Task not found");
    const set = s.deps.get(t.id)!;
    if (method === "GET")
      return json({
        key: t.key,
        dependsOn: keys(s, [...set]),
        blocks: keys(
          s,
          s.tasks.filter((x) => s.deps.get(x.id)?.has(t.id)).map((x) => x.id),
        ),
      });
    if (method === "POST") {
      if (
        !canWrite(
          role(
            s,
            s.projects.find((p) => p.id === t.projectId)!,
            uid,
          ),
        )
      )
        return fail(403, "forbidden", "Cannot add dependency");
      const other = taskByKey(s, body.dependsOn);
      if (!other || other.projectId !== t.projectId || other.id === t.id)
        return err("dependsOn");
      if (set.has(other.id)) return fail(409, "conflict", "Dependency exists");
      if (reaches(s, other.id, t.id))
        return fail(409, "dependency_cycle", "Dependency cycle");
      set.add(other.id);
      return json(
        { key: t.key, dependsOn: keys(s, [...set]), blocks: [] },
        201,
      );
    }
    if (method === "DELETE" && dep[2]) {
      const other = taskByKey(s, dep[2]);
      if (!other || !set.delete(other.id))
        return fail(404, "not_found", "Dependency not found");
      return new Response(null, { status: 204 });
    }
  }
  const m = path.match(/^\/tasks\/([^/]+)\/(history|comments)$/);
  if (m) {
    const t = taskByKey(s, m[1]!);
    if (!t || !member(s, t.projectId, uid))
      return fail(404, "not_found", "Task not found");
    if (m[2] === "history" && method === "GET")
      return json(s.history.get(t.id) ?? []);
    if (m[2] === "comments" && method === "GET")
      return json(s.comments.get(t.id) ?? []);
    if (m[2] === "comments" && method === "POST") {
      if (
        typeof body.body !== "string" ||
        !body.body.trim() ||
        body.body.length > 2000
      )
        return err("body");
      const item = {
        id: ++s.ids.comment,
        authorId: uid,
        body: body.body.trim(),
        createdAt: stamp(s),
      };
      const list = s.comments.get(t.id) ?? [];
      list.push(item);
      s.comments.set(t.id, list);
      return json(item, 201);
    }
  }
  return undefined;
}
function member(s: State, pid: number, uid: number) {
  const p = s.projects.find((p) => p.id === pid);
  return !!p && !!role(s, p, uid);
}
function keys(s: State, ids: number[]) {
  return ids
    .map((id) => s.tasks.find((t) => t.id === id)?.key)
    .filter(Boolean)
    .sort((a, b) => Number(a!.split("-")[1]) - Number(b!.split("-")[1]));
}
function reaches(
  s: State,
  from: number,
  to: number,
  seen = new Set<number>(),
): boolean {
  if (from === to) return true;
  if (seen.has(from)) return false;
  seen.add(from);
  return [...(s.deps.get(from) ?? [])].some((n) => reaches(s, n, to, seen));
}
