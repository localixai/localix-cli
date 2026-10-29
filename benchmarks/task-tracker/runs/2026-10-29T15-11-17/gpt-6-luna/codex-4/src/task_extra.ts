import type { State, Task } from "./model";
import { taskView } from "./model";
import { check, fail, memberRole, taskFor } from "./util";

function precondition(t: Task, h: Headers) {
  const value = h.get("If-Match");
  if (value && value !== `"${t.version}"`) fail(412, "precondition_failed", "task version changed");
}

export function listTasks(s: State, p: any, url: URL, uid: number) {
  const q = url.searchParams;
  const status = q.get("status"),
    sort = q.get("sort") || "created",
    limitText = q.get("limit") || "20";
  const limit = Number(limitText),
    assignee = q.get("assignee"),
    label = q.get("label")?.toLowerCase();
  check(!status || ["open", "in_progress", "done"].includes(status), "status");
  check(["created", "-created", "priority"].includes(sort), "sort");
  check(/^\d+$/.test(limitText) && limit >= 1 && limit <= 100, "limit");
  check(!assignee || ["me", "none"].includes(assignee) || /^\d+$/.test(assignee), "assignee");
  const all = s.tasks.filter(
    (t) =>
      t.projectId === p.id &&
      (!status || t.status === status) &&
      (!label || t.labels.includes(label)) &&
      (!assignee ||
        (assignee === "none"
          ? t.assigneeId === null
          : t.assigneeId === (assignee === "me" ? uid : Number(assignee)))) &&
      (!q.get("q") || `${t.title} ${t.description}`.toLowerCase().includes(q.get("q")!.toLowerCase())),
  );
  if (sort === "-created") all.reverse();
  if (sort === "priority")
    all.sort(
      (a, b) =>
        ["high", "normal", "low"].indexOf(a.priority) - ["high", "normal", "low"].indexOf(b.priority) ||
        a.number - b.number,
    );
  const cursor = q.get("cursor");
  let start = 0;
  if (cursor) {
    try {
      start = Number(atob(cursor));
      check(Number.isInteger(start) && start >= 0, "cursor");
    } catch {
      check(false, "cursor");
    }
  }
  const items = all.slice(start, start + limit);
  const nextCursor = start + limit < all.length ? btoa(String(start + limit)) : null;
  return {
    status: 200,
    body: { items: items.map((t) => taskView(t, p)), nextCursor },
  };
}
export function extras(
  s: State,
  method: string,
  sub: string,
  t: Task,
  p: any,
  b: Record<string, any>,
  uid: number,
  now: number,
  h: Headers,
) {
  if (sub === "history" && method === "GET") return { status: 200, body: s.history.get(t.id) };
  if (sub === "comments" && method === "GET") return { status: 200, body: s.comments.get(t.id) };
  if (sub === "comments" && method === "POST") {
    check(typeof b.body === "string" && b.body.trim().length > 0 && b.body.length <= 2000, "body");
    const item = {
      id: [...s.comments.values()].flat().length + 1,
      authorId: uid,
      body: b.body,
      createdAt: new Date(now).toISOString(),
    };
    s.comments.get(t.id)!.push(item);
    return { status: 201, body: item };
  }
  if (sub === "transitions" && method === "POST") {
    if (memberRole(s, p, uid) === "viewer") fail(403, "forbidden", "viewers cannot transition tasks");
    precondition(t, h);
    check(["open", "in_progress", "done"].includes(b.to), "to");
    const valid =
      (t.status === "open" && b.to === "in_progress") ||
      (t.status === "in_progress" && ["open", "done"].includes(b.to)) ||
      (t.status === "done" && b.to === "open");
    if (!valid) fail(409, "invalid_transition", "transition is not allowed");
    if (
      b.to === "done" &&
      [...(s.deps.get(t.id) || [])].some((id) => s.tasks.find((x) => x.id === id)?.status !== "done")
    )
      fail(409, "blocked", "dependencies are not done");
    s.history.get(t.id)!.push({
      at: new Date(now).toISOString(),
      by: uid,
      field: "status",
      from: t.status,
      to: b.to,
    });
    t.status = b.to;
    t.version++;
    t.updatedAt = new Date(now).toISOString();
    return { status: 200, body: taskView(t, p), etag: t.version };
  }
  const dm = /^dependencies(?:\/(.+))?$/.exec(sub);
  if (dm && method === "GET") {
    const deps = [...(s.deps.get(t.id) || [])].map((id) => s.tasks.find((x) => x.id === id)!).filter(Boolean);
    const blocks = [...s.deps]
      .filter(([, set]) => set.has(t.id))
      .map(([id]) => s.tasks.find((x) => x.id === id)!)
      .filter(Boolean);
    return {
      status: 200,
      body: {
        key: `${p.key}-${t.number}`,
        dependsOn: deps.sort((a, b) => a.number - b.number).map((x) => `${p.key}-${x.number}`),
        blocks: blocks.sort((a, b) => a.number - b.number).map((x) => `${p.key}-${x.number}`),
      },
    };
  }
  if (dm && method === "POST" && !dm[1]) {
    if (memberRole(s, p, uid) === "viewer") fail(403, "forbidden", "viewers cannot add dependencies");
    const other = taskFor(s, b.dependsOn);
    check(!!other && other.projectId === t.projectId && other.id !== t.id, "dependsOn");
    const set = s.deps.get(t.id)!;
    if (set.has(other.id)) fail(409, "conflict", "dependency already exists");
    const reaches = (id: number, seen = new Set<number>()): boolean =>
      id === t.id || (!seen.has(id) && (seen.add(id), [...(s.deps.get(id) || [])].some((next) => reaches(next, seen))));
    if (reaches(other.id)) fail(409, "dependency_cycle", "dependency would create a cycle");
    set.add(other.id);
    const list = [...set].map((id) => s.tasks.find((x) => x.id === id)!).sort((a, b) => a.number - b.number);
    return {
      status: 201,
      body: {
        key: `${p.key}-${t.number}`,
        dependsOn: list.map((x) => `${p.key}-${x.number}`),
        blocks: [],
      },
    };
  }
  if (dm?.[1] && method === "DELETE") {
    const other = taskFor(s, dm[1]);
    if (!other || !s.deps.get(t.id)!.delete(other.id)) fail(404, "not_found", "dependency not found");
    return { status: 204, body: null };
  }
}
