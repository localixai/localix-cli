import { State, visibleTask, bad, error, notFound } from "./model";
export function dependencyRoute(s: State, method: string, path: string, user: number,
  data: Record<string, any> | null): Response | undefined {
  const m = path.match(/^\/tasks\/([A-Z]+-\d+)\/dependencies(?:\/([A-Z]+-\d+))?$/);
  if (!m) return;
  const t = visibleTask(s, m[1]!, user);
  if (!t) return notFound();
  const keys = (ids: number[]) => ids.map(id => s.tasks.find(x => x.id === id)!)
    .sort((a, b) => a.number - b.number).map(x => x.key);
  const own = s.deps.get(t.id)!;
  if (method === "GET") return Response.json({ key: t.key,
    dependsOn: keys([...own]), blocks: keys([...s.deps].filter(([, set]) => set.has(t.id)).map(([id]) => id)) });
  if (method === "POST") {
    const project = s.projects.find(p => p.id === t.projectId)!;
    if (s.members.get(project.id)?.get(user) === "viewer")
      return error("forbidden", "Forbidden", 403);
    const on = typeof data?.dependsOn === "string" ? visibleTask(s, data.dependsOn, user) : undefined;
    if (!on || on.projectId !== t.projectId || on.id === t.id) return bad("dependsOn");
    if (own.has(on.id)) return error("conflict", "Dependency exists", 409);
    const reaches = (id: number, target: number, seen = new Set<number>()): boolean => {
      if (id === target) return true;
      if (seen.has(id)) return false;
      seen.add(id);
      return [...(s.deps.get(id) ?? [])].some(next => reaches(next, target, seen));
    };
    if (reaches(on.id, t.id)) return error("dependency_cycle", "Dependency cycle", 409);
    own.add(on.id);
    return Response.json({ key: t.key, dependsOn: keys([...own]), blocks: [] }, { status: 201 });
  }
  if (method === "DELETE" && m[2]) {
    const on = s.tasks.find(x => x.key === m[2]);
    if (!on || !own.delete(on.id)) return notFound();
    return new Response(null, { status: 204 });
  }
}
