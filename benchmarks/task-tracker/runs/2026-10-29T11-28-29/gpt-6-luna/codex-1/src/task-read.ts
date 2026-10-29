import {
  fail,
  json,
  projectAccess,
  role,
  taskByKey,
  taskView,
  type State,
} from "./model";
export function readTask(
  s: State,
  method: string,
  path: string,
  uid: number,
  url: URL,
) {
  const m = path.match(/^\/projects\/(\d+)\/tasks$/);
  if (m && method === "GET") {
    const p = projectAccess(s, uid, Number(m[1]));
    if (!p) return fail(404, "not_found", "Project not found");
    let items = s.tasks.filter((t) => t.projectId === p.id && !t.deleted);
    const q = url.searchParams;
    if (q.has("status")) {
      const v = q.get("status")!;
      if (!["open", "in_progress", "done"].includes(v))
        return fail(400, "validation", "Invalid status", "status");
      items = items.filter((t) => t.status === v);
    }
    if (q.has("assignee")) {
      const v = q.get("assignee")!;
      if (v === "me") items = items.filter((t) => t.assigneeId === uid);
      else if (v === "none") items = items.filter((t) => t.assigneeId === null);
      else if (/^\d+$/.test(v))
        items = items.filter((t) => t.assigneeId === Number(v));
      else return fail(400, "validation", "Invalid assignee", "assignee");
    }
    if (q.has("label"))
      items = items.filter((t) =>
        t.labels.includes(q.get("label")!.toLowerCase()),
      );
    if (q.has("q")) {
      const v = q.get("q")!.toLowerCase();
      items = items.filter((t) =>
        (t.title + " " + t.description).toLowerCase().includes(v),
      );
    }
    const sort = q.get("sort") ?? "created";
    if (!["created", "-created", "priority"].includes(sort))
      return fail(400, "validation", "Invalid sort", "sort");
    items.sort((a, b) => {
      if (sort === "priority") {
        const rank = { high: 0, normal: 1, low: 2 };
        return (
          rank[a.priority as keyof typeof rank] -
            rank[b.priority as keyof typeof rank] || a.number - b.number
        );
      }
      return sort === "-created" ? b.number - a.number : a.number - b.number;
    });
    let limit = 20;
    if (q.has("limit")) {
      const n = Number(q.get("limit"));
      if (!Number.isInteger(n) || n < 1 || n > 100)
        return fail(400, "validation", "Invalid limit", "limit");
      limit = n;
    }
    let offset = 0;
    if (q.has("cursor")) {
      const n = Number(q.get("cursor"));
      if (!Number.isInteger(n) || n < 0)
        return fail(400, "validation", "Invalid cursor", "cursor");
      offset = n;
    }
    const page = items.slice(offset, offset + limit);
    const next = offset + limit < items.length ? String(offset + limit) : null;
    return json({ items: page.map(taskView), nextCursor: next });
  }
  const m2 = path.match(/^\/tasks\/([^/]+)$/);
  if (m2) {
    const t = taskByKey(s, m2[1]!);
    if (!t || !roleForTask(s, t.projectId, uid))
      return fail(404, "not_found", "Task not found");
    return method === "GET"
      ? json(taskView(t), 200, { ETag: `"${t.version}"` })
      : undefined;
  }
  return undefined;
}
function roleForTask(s: State, id: number, uid: number) {
  const p = s.projects.find((x) => x.id === id);
  return p && role(s, p, uid);
}
