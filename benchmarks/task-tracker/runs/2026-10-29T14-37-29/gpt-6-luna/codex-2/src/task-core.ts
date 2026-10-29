import {
  Context,
  fail,
  json,
  nextId,
  iso,
  memberRole,
  visibleProject,
  taskByKey,
  Task,
} from "./model";
function labels(value: any) {
  if (
    !Array.isArray(value) ||
    value.some(
      (x) =>
        typeof x !== "string" ||
        !x.trim() ||
        x.trim().length > 30 ||
        /\s/.test(x.trim()),
    )
  )
    return null;
  return [...new Set(value.map((x: string) => x.trim().toLowerCase()))].sort();
}
export function createTask(c: Context, p: any, body: any) {
  const title = typeof body.title === "string" ? body.title.trim() : "";
  const description = body.description === undefined ? "" : body.description;
  const priority = body.priority ?? "normal",
    labs = body.labels === undefined ? [] : labels(body.labels);
  if (!title || title.length > 200)
    return fail("validation", "invalid title", 400, "title");
  if (typeof description !== "string")
    return fail("validation", "invalid description", 400, "description");
  if (!["low", "normal", "high"].includes(priority))
    return fail("validation", "invalid priority", 400, "priority");
  if (labs === null) return fail("validation", "invalid labels", 400, "labels");
  if (body.assigneeId != null && !memberRole(c.state, p.id, body.assigneeId))
    return fail("validation", "invalid assignee", 400, "assigneeId");
  const n = nextId(c.state, `task-${p.id}`),
    at = iso(c);
  const t: Task = {
    id: nextId(c.state, "task"),
    key: `${p.key}-${n}`,
    number: n,
    projectId: p.id,
    title,
    description,
    status: "open",
    priority,
    labels: labs,
    assigneeId: body.assigneeId ?? null,
    createdBy: c.userId!,
    createdAt: at,
    updatedAt: at,
    version: 1,
  };
  c.state.tasks.push(t);
  c.state.histories.set(t.id, []);
  c.state.comments.set(t.id, []);
  c.state.deps.set(t.id, new Set());
  return json(t, 201, { ETag: '"1"' });
}
export function listTasks(c: Context, p: any, url: URL) {
  const q = url.searchParams,
    status = q.get("status"),
    assignee = q.get("assignee"),
    label = q.get("label"),
    search = q.get("q")?.toLowerCase();
  const sort = q.get("sort") ?? "created",
    limitRaw = q.get("limit") ?? "20",
    limit = Number(limitRaw);
  if (status && !["open", "in_progress", "done"].includes(status))
    return fail("validation", "invalid status", 400, "status");
  if (!["created", "-created", "priority"].includes(sort))
    return fail("validation", "invalid sort", 400, "sort");
  if (!/^\d+$/.test(limitRaw) || limit < 1 || limit > 100)
    return fail("validation", "invalid limit", 400, "limit");
  if (
    assignee &&
    assignee !== "me" &&
    assignee !== "none" &&
    !/^\d+$/.test(assignee)
  )
    return fail("validation", "invalid assignee", 400, "assignee");
  let items = c.state.tasks.filter((t) => t.projectId === p.id);
  if (status) items = items.filter((t) => t.status === status);
  if (assignee)
    items = items.filter(
      (t) =>
        t.assigneeId ===
        (assignee === "me"
          ? c.userId
          : assignee === "none"
            ? null
            : Number(assignee)),
    );
  if (label)
    items = items.filter((t) => t.labels.includes(label.toLowerCase()));
  if (search)
    items = items.filter((t) =>
      `${t.title} ${t.description}`.toLowerCase().includes(search),
    );
  items.sort((a, b) =>
    sort === "-created"
      ? b.number - a.number
      : sort === "priority"
        ? ["high", "normal", "low"].indexOf(a.priority) -
            ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number
        : a.number - b.number,
  );
  let start = 0,
    cursor = q.get("cursor");
  if (cursor) {
    try {
      start = Number(atob(cursor));
      if (!Number.isInteger(start) || start < 0) throw 0;
    } catch {
      return fail("validation", "invalid cursor", 400, "cursor");
    }
  }
  const page = items.slice(start, start + limit),
    next = start + limit < items.length ? btoa(String(start + limit)) : null;
  return json({ items: page, nextCursor: next });
}
export function change(c: Context, t: Task, body: any, request: Request) {
  const allowed = ["title", "description", "priority", "labels", "assigneeId"];
  for (const key of Object.keys(body))
    if (!allowed.includes(key))
      return fail("validation", "unknown field", 400, key);
  const etag = request.headers.get("If-Match");
  if (etag && etag !== `"${t.version}"`)
    return fail("precondition_failed", "task changed", 412);
  const values: any = { ...body };
  if ("title" in values) {
    if (
      typeof values.title !== "string" ||
      !values.title.trim() ||
      values.title.trim().length > 200
    )
      return fail("validation", "invalid title", 400, "title");
    values.title = values.title.trim();
  }
  if ("description" in values && typeof values.description !== "string")
    return fail("validation", "invalid description", 400, "description");
  if (
    "priority" in values &&
    !["low", "normal", "high"].includes(values.priority)
  )
    return fail("validation", "invalid priority", 400, "priority");
  if ("labels" in values) {
    values.labels = labels(values.labels);
    if (!values.labels)
      return fail("validation", "invalid labels", 400, "labels");
  }
  if (
    "assigneeId" in values &&
    values.assigneeId !== null &&
    !memberRole(c.state, t.projectId, values.assigneeId)
  )
    return fail("validation", "invalid assignee", 400, "assigneeId");
  const changed: string[] = [];
  for (const key of allowed)
    if (
      key in values &&
      JSON.stringify((t as any)[key]) !== JSON.stringify(values[key])
    )
      changed.push(key);
  changed.sort();
  if (changed.length) {
    for (const field of changed)
      c.state.histories.get(t.id)!.push({
        at: iso(c),
        by: c.userId,
        field,
        from: (t as any)[field],
        to: values[field],
      });
    for (const field of changed) (t as any)[field] = values[field];
    t.version++;
    t.updatedAt = iso(c);
  }
  return json(t, 200, { ETag: `"${t.version}"` });
}
