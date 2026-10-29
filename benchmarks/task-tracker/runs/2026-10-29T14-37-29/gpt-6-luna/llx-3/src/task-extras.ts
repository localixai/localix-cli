import type { State } from "./state";
import { bad, body, err, iso, json, record, taskReply } from "./util";
import { permission } from "./task-core";
import { transition, update } from "./task-changes";

export async function taskExtras(req: Request, s: State, u: any, t: any, p: any, sub: string) {
  const role = permission(p, u), method = req.method;
  if (!sub && method === "GET") return taskReply(t);
  if (sub === "history" && method === "GET")
    return json(s.history.filter(x => x.taskId === t.id).map(({ taskId, ...x }) => x));
  if (sub === "comments") {
    if (method === "GET") return json(s.comments.filter(c => c.taskId === t.id).map(({ taskId, ...c }) => c));
    if (method === "POST") {
      const b = await body(req); if (!b) return err(400, "validation", "Invalid JSON");
      if (typeof b.body !== "string" || !b.body.trim() || b.body.length > 2000) return bad("body");
      const c = { id: s.comments.length + 1, taskId: t.id, authorId: u.id, body: b.body.trim(),
        createdAt: iso(s) }; s.comments.push(c);
      const { taskId, ...view } = c; return json(view, 201);
    }
  }
  if (sub === "dependencies") {
    const deps = [...s.dependencies].filter(x => x.startsWith(`${t.id}:`)).map(x => Number(x.split(":")[1]));
    const blocks = [...s.dependencies].filter(x => x.endsWith(`:${t.id}`)).map(x => Number(x.split(":")[0]));
    const keys = (ids: number[]) => ids.map(id => s.tasks.find(x => x.id === id)).filter(Boolean)
      .sort((a, b) => a.number - b.number).map(x => x.key);
    if (method === "GET") return json({ key: t.key, dependsOn: keys(deps), blocks: keys(blocks) });
    if (method === "POST") {
      if (!["owner", "member"].includes(role)) return err(403, "forbidden", "Cannot add dependency");
      const b = await body(req); if (!b) return err(400, "validation", "Invalid JSON");
      const target = typeof b.dependsOn === "string" ?
        s.tasks.find(x => x.key === b.dependsOn && !x.deleted && x.projectId === t.projectId) : null;
      if (!target || target.id === t.id) return bad("dependsOn");
      const edge = `${t.id}:${target.id}`;
      if (s.dependencies.has(edge)) return err(409, "conflict", "Dependency exists");
      if (reaches(s, target.id, t.id)) return err(409, "dependency_cycle", "Dependency cycle");
      s.dependencies.add(edge);
      return json({ key: t.key, dependsOn: keys([...deps, target.id]), blocks: keys(blocks) }, 201);
    }
  }
  const depDelete = sub.match(/^dependencies\/([^/]+)$/);
  if (depDelete && method === "DELETE") {
    const target = s.tasks.find(x => x.key === decodeURIComponent(depDelete[1]!));
    const edge = target && `${t.id}:${target.id}`;
    if (!edge || !s.dependencies.has(edge)) return err(404, "not_found", "Dependency not found");
    s.dependencies.delete(edge); return new Response(null, { status: 204 });
  }
  if (sub === "transitions" && method === "POST") return transition(req, s, u, t);
  if (!sub && method === "PATCH") return update(req, s, u, t, p);
  if (!sub && method === "DELETE") {
    if (role !== "owner" && !(role === "member" && t.createdBy === u.id))
      return err(403, "forbidden", "Cannot delete task");
    t.deleted = true;
    for (const e of [...s.dependencies]) if (e.split(":").includes(String(t.id))) s.dependencies.delete(e);
    return new Response(null, { status: 204 });
  }
  return null;
}
function reaches(s: State, from: number, target: number): boolean {
  if (from === target) return true;
  return [...s.dependencies].filter(x => x.startsWith(`${from}:`))
    .some(x => reaches(s, Number(x.split(":")[1]), target));
}
