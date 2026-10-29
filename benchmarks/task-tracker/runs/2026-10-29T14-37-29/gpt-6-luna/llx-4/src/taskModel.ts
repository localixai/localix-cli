export function valid(body: any): string | null {
  if (typeof body.title !== "string" || !body.title.trim() || body.title.length > 200) return "title";
  if (body.description !== undefined && typeof body.description !== "string") return "description";
  if (body.priority !== undefined && !["low", "normal", "high"].includes(body.priority)) return "priority";
  if (body.labels !== undefined && !Array.isArray(body.labels)) return "labels";
  if (Array.isArray(body.labels) && body.labels.some((x: any) =>
    typeof x !== "string" || !x.trim() || x.trim().length > 30 || /\s/.test(x.trim()))) return "labels";
  return null;
}

export function taskPayload(body: any, project: any, tasks: any[], now: number): any {
  const labels = [...new Set((body.labels ?? []).map((x: string) => x.trim().toLowerCase()))].sort();
  const item = {
    id: tasks.length + 1, key: `${project.key}-${project.next = (project.next ?? 0) + 1}`,
    number: project.next, projectId: project.id, title: body.title.trim(), description: body.description ?? "",
    status: "open", priority: body.priority ?? "normal", labels,
    assigneeId: body.assigneeId ?? null, creatorId: body.creatorId,
    createdAt: new Date(now).toISOString(), updatedAt: new Date(now).toISOString(),
    version: 1, history: [], comments: [], dependencies: [],
  };
  tasks.push(item);
  return item;
}

export function publicTask(task: any): any {
  const { history: _h, comments: _c, dependencies: _d, creatorId, ...item } = task;
  return { ...item, createdBy: creatorId };
}
