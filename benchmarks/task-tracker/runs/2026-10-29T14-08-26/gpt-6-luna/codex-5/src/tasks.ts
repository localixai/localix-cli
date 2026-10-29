import type { Ctx } from "./context";
import { comments, dependencies, patch, transition } from "./taskOps";
import { iso, project, task, taskDto, write } from "./context";
import { bad, fail, json } from "./http";
import type { Project, Task } from "./model";
const priorities = ["low", "normal", "high"];
export function validate(b: any, p: Project, c: Ctx, prefix = ""): any {
  const title = typeof b.title === "string" ? b.title.trim() : "";
  if (!title || title.length > 200) bad(prefix + "title");
  const description = b.description === undefined ? "" : b.description;
  if (typeof description !== "string") bad(prefix + "description");
  const priority = b.priority ?? "normal";
  if (!priorities.includes(priority)) bad(prefix + "priority");
  const labels = b.labels ?? [];
  if (
    !Array.isArray(labels) ||
    labels.some(
      (x: any) => typeof x !== "string" || !/^[\w-]{1,30}$/.test(x.trim()),
    )
  )
    bad(prefix + "labels");
  const assigneeId = b.assigneeId ?? null;
  if (assigneeId !== null && !p.members.has(assigneeId))
    bad(prefix + "assigneeId");
  return {
    title,
    description,
    priority,
    labels: [
      ...new Set(labels.map((x: string) => x.trim().toLowerCase())),
    ].sort(),
    assigneeId,
  };
}
function make(c: Ctx, p: Project, b: any): Task {
  const v = validate(b, p, c);
  const n = p.next++;
  const t: Task = {
    id: c.s.tasks.length + 1,
    key: `${p.key}-${n}`,
    number: n,
    projectId: p.id,
    ...v,
    status: "open",
    createdBy: c.user.id,
    createdAt: iso(c),
    updatedAt: iso(c),
    version: 1,
    deleted: false,
    deps: new Set(),
    history: [],
    comments: [],
  };
  c.s.tasks.push(t);
  return t;
}
function etag(t: Task) {
  return { ETag: `"${t.version}"` };
}
export function taskRoute(
  c: Ctx,
  path: string,
  method: string,
): Response | undefined {
  const bulk = path.match(/^\/projects\/(\d+)\/tasks\/bulk$/);
  const listing = path.match(/^\/projects\/(\d+)\/tasks$/);
  if (bulk && method === "POST") {
    const p = project(c, bulk[1]!);
    write(p, c.user.id);
    if (
      !Array.isArray(c.body.tasks) ||
      c.body.tasks.length < 1 ||
      c.body.tasks.length > 50
    )
      bad("tasks");
    const vals = c.body.tasks.map((x: any, i: number) =>
      validate(x, p, c, `tasks[${i}].`),
    );
    const out = vals.map((_: any, i: number) => make(c, p, c.body.tasks[i]));
    return json({ items: out.map(taskDto) }, 201);
  }
  if (listing && method === "POST") {
    const p = project(c, listing[1]!);
    write(p, c.user.id);
    const idem = c.req.headers.get("Idempotency-Key");
    const ik = `${c.user.id}:${p.id}:${idem}`;
    if (idem) {
      const old = c.s.idem.get(ik);
      if (old && old.until > c.now()) {
        if (old.body !== JSON.stringify(c.body))
          fail(422, "idempotency_mismatch", "Idempotency key mismatch");
        return json(old.task, 201, {
          "Idempotent-Replayed": "true",
          ETag: `"${old.task.version}"`,
        });
      }
    }
    const out = make(c, p, c.body);
    if (idem)
      c.s.idem.set(ik, {
        body: JSON.stringify(c.body),
        task: taskDto(out),
        until: c.now() + 86400000,
      });
    return json(taskDto(out), 201, etag(out));
  }
  if (listing && method === "GET") {
    const p = project(c, listing[1]!);
    const q = c.url.searchParams;
    let rows = c.s.tasks.filter((t) => t.projectId === p.id && !t.deleted);
    for (const [k, v] of q) {
      if (k === "status") {
        if (!["open", "in_progress", "done"].includes(v)) bad(k);
        rows = rows.filter((t) => t.status === v);
      }
      if (k === "label")
        rows = rows.filter((t) => t.labels.includes(v.toLowerCase()));
      if (k === "q")
        rows = rows.filter((t) =>
          `${t.title} ${t.description}`.toLowerCase().includes(v.toLowerCase()),
        );
      if (k === "assignee") {
        if (v === "none") rows = rows.filter((t) => t.assigneeId === null);
        else if (v === "me")
          rows = rows.filter((t) => t.assigneeId === c.user.id);
        else if (/^\d+$/.test(v))
          rows = rows.filter((t) => t.assigneeId === Number(v));
        else bad(k);
      }
    }
    const sort = q.get("sort") ?? "created";
    if (!["created", "-created", "priority"].includes(sort)) bad("sort");
    rows.sort(
      sort === "-created"
        ? (a, b) => b.number - a.number
        : sort === "priority"
          ? (a, b) =>
              priorities.indexOf(b.priority) - priorities.indexOf(a.priority) ||
              a.number - b.number
          : (a, b) => a.number - b.number,
    );
    const limitText = q.get("limit") ?? "20",
      limit = Number(limitText);
    if (!/^\d+$/.test(limitText) || limit < 1 || limit > 100) bad("limit");
    let offset = 0;
    const cur = q.get("cursor");
    if (cur) {
      try {
        offset = Number(atob(cur));
        if (!Number.isInteger(offset) || offset < 0) bad("cursor");
      } catch {
        bad("cursor");
      }
    }
    const items = rows.slice(offset, offset + limit);
    return json({
      items: items.map(taskDto),
      nextCursor:
        offset + limit < rows.length ? btoa(String(offset + limit)) : null,
    });
  }
  const match = path.match(/^\/tasks\/([^/]+)(?:\/(.*))?$/);
  if (!match) return undefined;
  const t = task(c, match[1]!);
  const sub = match[2] ?? "";
  const p = c.s.projects.find((x) => x.id === t.projectId)!;
  if (!sub && method === "GET") return json(taskDto(t), 200, etag(t));
  if (
    ["PATCH", "DELETE"].includes(method) ||
    (sub === "transitions" && method === "POST")
  )
    write(p, c.user.id);
  if (
    ((!sub && method === "PATCH") ||
      (sub === "transitions" && method === "POST")) &&
    c.req.headers.has("If-Match") &&
    c.req.headers.get("If-Match") !== `"${t.version}"`
  )
    fail(412, "precondition_failed", "Task changed");
  if (!sub && method === "PATCH") return patch(c, t, p);
  if (!sub && method === "DELETE") {
    if (p.ownerId !== c.user.id && t.createdBy !== c.user.id)
      fail(403, "forbidden", "Forbidden");
    t.deleted = true;
    for (const x of c.s.tasks) x.deps.delete(t.key);
    return new Response(null, { status: 204 });
  }
  if (sub === "transitions" && method === "POST") return transition(c, t);
  if (sub === "history" && method === "GET") return json(t.history);
  if (sub === "comments") return comments(c, t, method);
  if (sub === "dependencies") return dependencies(c, t, method, "");
  const dep = sub.match(/^dependencies\/([^/]+)$/);
  if (dep) return dependencies(c, t, method, dep[1]!);
  return undefined;
}
