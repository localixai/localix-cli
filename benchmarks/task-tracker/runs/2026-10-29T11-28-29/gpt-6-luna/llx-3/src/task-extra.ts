import { State, Task, bad, byTaskKey, error, iso, json, memberRole, projectForTask } from "./model";
export function extraTask(s: State, method: string, path: string, body: any, uid: number, task: Task) {
  const p = projectForTask(s, task), role = memberRole(s, p, uid);
  if (!role) return error("not_found", "Task not found", 404);
  if (path.endsWith("/comments")) {
    if (method === "GET") return json(s.comments.get(task.id));
    if (method === "POST") {
      if (typeof body.body !== "string" || !body.body.trim() || body.body.length > 2000) return bad("body");
      const list = s.comments.get(task.id)!;
      const item = { id: list.length + 1, authorId: uid, body: body.body.trim(), createdAt: iso(s) };
      list.push(item); return json(item, 201);
    }
  }
  if (path.endsWith("/history") && method === "GET") return json(s.history.get(task.id));
  if (path.endsWith("/dependencies")) {
    if (method === "GET") {
      const ids = s.deps.get(task.id)!;
      const names = (xs: Task[]) => xs.sort((a,b) => a.number-b.number).map((t) => t.key);
      return json({ key: task.key,
        dependsOn: names([...ids].map((id) => s.tasks.find((t) => t.id === id)!).filter(Boolean)),
        blocks: names(s.tasks.filter((t) => s.deps.get(t.id)?.has(task.id))) });
    }
    if (method === "POST") {
      if (role === "viewer") return error("forbidden", "Forbidden", 403);
      const other = byTaskKey(s, body.dependsOn);
      if (!other || other.projectId !== task.projectId || other.id === task.id) return bad("dependsOn");
      const deps = s.deps.get(task.id)!;
      if (deps.has(other.id)) return error("conflict", "Dependency exists", 409);
      const reaches = (id: number, seen = new Set<number>()): boolean => id === task.id ||
        (!seen.has(id) && (seen.add(id), [...(s.deps.get(id) || [])].some((x) => reaches(x, seen))));
      if (reaches(other.id)) return error("dependency_cycle", "Dependency cycle", 409);
      deps.add(other.id);
      return json({ key: task.key,
        dependsOn: [...deps].map((id) => s.tasks.find((t) => t.id === id)!.key).sort(),
        blocks: [] }, 201);
    }
  }
}
