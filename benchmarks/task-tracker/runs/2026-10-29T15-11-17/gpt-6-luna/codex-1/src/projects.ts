import {
  body,
  fail,
  iso,
  json,
  person,
  projectFor,
  validate,
  type State,
} from "./model";
export async function projects(
  req: Request,
  path: string,
  url: URL,
  s: State,
  u: number,
) {
  if (path === "/projects" && req.method === "POST") {
    const b = await body(req),
      name = typeof b.name === "string" ? b.name.trim() : "";
    validate(name.length > 0 && name.length <= 100, "name");
    validate(typeof b.key === "string" && /^[A-Z]{2,6}$/.test(b.key), "key");
    if (s.projects.some((p) => p.key === b.key))
      fail(409, "conflict", "key already used", "key");
    const p = {
      id: ++s.counters.project,
      name,
      key: b.key,
      ownerId: u,
      createdAt: iso(s),
      seq: 0,
      members: new Map<number, string>([[u, "owner"]]),
    };
    s.projects.push(p);
    return json(
      { id: p.id, name, key: p.key, ownerId: u, createdAt: p.createdAt },
      201,
    );
  }
  if (path === "/projects" && req.method === "GET")
    return json(
      s.projects
        .filter((p) => p.members.has(u))
        .map(({ id, name, key, ownerId, createdAt }) => ({
          id,
          name,
          key,
          ownerId,
          createdAt,
        })),
    );
  const m = path.match(
    /^\/projects\/([^/]+)(?:\/(members|tasks|stats)(?:\/([^/]+))?)?$/,
  );
  if (!m) return undefined;
  const id = Number(m[1]);
  if (!Number.isInteger(id)) fail(404, "not_found");
  const p = projectFor(s, id, u),
    sub = m[2];
  if (!sub && req.method === "GET")
    return json({
      id: p.id,
      name: p.name,
      key: p.key,
      ownerId: p.ownerId,
      createdAt: p.createdAt,
    });
  if (sub === "members" && m[3] && req.method === "DELETE") {
    if (u !== p.ownerId) fail(403, "forbidden");
    const uid = Number(m[3]);
    if (!p.members.has(uid) || uid === p.ownerId) fail(404, "not_found");
    p.members.delete(uid);
    for (const t of s.tasks)
      if (t.projectId === id && t.assigneeId === uid) t.assigneeId = null;
    return new Response(null, { status: 204 });
  }
  if (sub === "members" && req.method === "GET")
    return json(
      [...p.members]
        .sort((a, b) => a[0] - b[0])
        .map(([userId, role]) => ({ userId, role })),
    );
  if (sub === "members" && req.method === "POST") {
    if (u !== p.ownerId) fail(403, "forbidden");
    const b = await body(req);
    validate(b.role === "member" || b.role === "viewer", "role");
    validate(
      s.users.some((x) => x.id === b.userId),
      "userId",
    );
    if (p.members.has(b.userId)) fail(409, "conflict");
    p.members.set(b.userId, b.role);
    return json({ userId: b.userId, role: b.role }, 201);
  }
  if (sub === "tasks" && req.method === "GET") return taskList(url, s, p, u);
  if (sub === "stats" && req.method === "GET") {
    const ts = s.tasks.filter((t) => t.projectId === id);
    const byStatus = { open: 0, in_progress: 0, done: 0 },
      byPriority = { high: 0, normal: 0, low: 0 },
      byAssignee = new Map<number, number>();
    for (const t of ts) {
      (byStatus as any)[t.status]++;
      (byPriority as any)[t.priority]++;
      if (t.assigneeId !== null)
        byAssignee.set(t.assigneeId, (byAssignee.get(t.assigneeId) || 0) + 1);
    }
    return json({
      total: ts.length,
      byStatus,
      byPriority,
      byAssignee: [...byAssignee]
        .sort((a, b) => b[1] - a[1] || a[0] - b[0])
        .map(([userId, count]) => ({ userId, count })),
    });
  }
  return undefined;
}
function taskList(url: URL, s: State, p: any, u: number) {
  const q = url.searchParams,
    status = q.get("status"),
    sort = q.get("sort") || "created",
    limitRaw = q.get("limit") || "20",
    limit = Number(limitRaw),
    assignee = q.get("assignee"),
    cursor = q.get("cursor");
  if (status && !["open", "in_progress", "done"].includes(status))
    fail(400, "validation", "invalid status", "status");
  if (!["created", "-created", "priority"].includes(sort))
    fail(400, "validation", "invalid sort", "sort");
  if (!/^\d+$/.test(limitRaw) || limit < 1 || limit > 100)
    fail(400, "validation", "invalid limit", "limit");
  if (
    assignee &&
    !(["me", "none"].includes(assignee) || /^\d+$/.test(assignee))
  )
    fail(400, "validation", "invalid assignee", "assignee");
  let all = s.tasks.filter((t) => t.projectId === p.id);
  if (status) all = all.filter((t) => t.status === status);
  if (assignee === "me") all = all.filter((t) => t.assigneeId === u);
  if (assignee === "none") all = all.filter((t) => t.assigneeId === null);
  if (assignee && /^\d+$/.test(assignee))
    all = all.filter((t) => t.assigneeId === Number(assignee));
  const label = q.get("label")?.toLowerCase(),
    text = q.get("q")?.toLowerCase();
  if (label) all = all.filter((t) => t.labels.includes(label));
  if (text)
    all = all.filter((t) =>
      (t.title + " " + t.description).toLowerCase().includes(text),
    );
  all.sort(
    sort === "-created"
      ? (a, b) => b.number - a.number
      : sort === "priority"
        ? (a, b) =>
            ["high", "normal", "low"].indexOf(a.priority) -
              ["high", "normal", "low"].indexOf(b.priority) ||
            a.number - b.number
        : (a, b) => a.number - b.number,
  );
  let start = 0;
  if (cursor) {
    try {
      const n = Number(atob(cursor));
      if (!Number.isInteger(n) || n < 0) throw 0;
      start = n;
    } catch {
      fail(400, "validation", "invalid cursor", "cursor");
    }
  }
  const items = all.slice(start, start + limit),
    next = start + limit < all.length ? btoa(String(start + limit)) : null;
  return json({ items, nextCursor: next });
}
