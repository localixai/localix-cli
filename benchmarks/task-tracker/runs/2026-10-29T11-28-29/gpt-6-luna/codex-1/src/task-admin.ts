import {
  fail,
  json,
  projectAccess,
  role,
  taskByKey,
  type State,
} from "./model";
export function deleteOrStats(
  s: State,
  method: string,
  path: string,
  uid: number,
): Response | undefined {
  const m = path.match(/^\/tasks\/([^/]+)$/);
  if (m && method === "DELETE") {
    const t = taskByKey(s, m[1]!);
    if (!t) return fail(404, "not_found", "Task not found");
    const p = s.projects.find((x) => x.id === t.projectId)!;
    const r = role(s, p, uid);
    if (!r) return fail(404, "not_found", "Task not found");
    if (r !== "owner" && t.createdBy !== uid)
      return fail(403, "forbidden", "Cannot delete task");
    t.deleted = true;
    for (const deps of s.deps.values()) deps.delete(t.id);
    s.deps.delete(t.id);
    return new Response(null, { status: 204 });
  }
  const st = path.match(/^\/projects\/(\d+)\/stats$/);
  if (st && method === "GET") {
    const p = projectAccess(s, uid, Number(st[1]));
    if (!p) return fail(404, "not_found", "Project not found");
    const ts = s.tasks.filter((t) => t.projectId === p.id && !t.deleted);
    const count = (field: string, values: string[]) =>
      Object.fromEntries(
        values.map((v) => [
          v,
          ts.filter((t) => (t as any)[field] === v).length,
        ]),
      );
    const assignees = new Map<number, number>();
    for (const t of ts)
      if (t.assigneeId !== null)
        assignees.set(t.assigneeId, (assignees.get(t.assigneeId) ?? 0) + 1);
    return json({
      total: ts.length,
      byStatus: count("status", ["open", "in_progress", "done"]),
      byPriority: count("priority", ["high", "normal", "low"]),
      byAssignee: [...assignees]
        .sort((a, b) => b[1] - a[1] || a[0] - b[0])
        .map(([userId, count]) => ({ userId, count })),
    });
  }
  return undefined;
}
