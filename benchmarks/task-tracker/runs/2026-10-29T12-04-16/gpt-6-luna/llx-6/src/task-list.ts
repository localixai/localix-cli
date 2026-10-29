import { type Ctx, auth, body, fail, ok, valid } from "./http";
import { makeTask, view } from "./task-core";
export async function taskLists(c: Ctx, path: string, method: string): Promise<Response | null> {
  const m = path.match(/^\/projects\/(\d+)\/tasks(?:\/bulk)?$/);
  if (!m) return null;
  const pid = Number(m[1]), user = auth(c, method === "GET" ? undefined : "member", pid);
  const bulk = path.endsWith("/bulk");
  if (method === "POST") {
    const idem = c.request.headers.get("Idempotency-Key");
    const b = await body(c);
    if (bulk) {
      valid(Array.isArray(b.tasks) && b.tasks.length > 0 && b.tasks.length <= 50, "tasks");
      const made = [];
      for (let i = 0; i < b.tasks.length; i++) {
        try { validate(c, pid, b.tasks[i]); }
        catch (e: any) { if (e.field) e.field = `tasks[${i}].${e.field}`; throw e; }
      }
      for (const item of b.tasks) made.push(view(makeTask(c, pid, item, user)));
      return ok({ items: made }, 201);
    }
    const raw = JSON.stringify(b), ik = `${user}:${idem}`;
    if (idem) {
      const old = c.s.idem.get(ik);
      if (old && c.s.now() - old.at < 86400000) {
        if (old.body !== raw) fail(422, "idempotency_mismatch", "Key used with another body");
        return ok(view(old.task), 201, { "Idempotent-Replayed": "true", ETag: `"${old.task.version}"` });
      }
    }
    validate(c, pid, b);
    if (idem) {
      const old = c.s.idem.get(ik);
      if (old && c.s.now() - old.at >= 86400000) c.s.idem.delete(ik);
    }
    const t = makeTask(c, pid, b, user);
    if (idem) c.s.idem.set(ik, { at: c.s.now(), body: raw, task: t });
    return ok(view(t), 201, { ETag: `"1"` });
  }
  if (method !== "GET") return null;
  let tasks = c.s.tasks.filter((t) => t.projectId === pid);
  const q = c.url.searchParams;
  const status = q.get("status"), assignee = q.get("assignee"), label = q.get("label"), search = q.get("q");
  if (status) {
    valid(["open", "in_progress", "done"].includes(status), "status");
    tasks = tasks.filter((t) => t.status === status);
  }
  if (assignee) {
    valid(["me", "none"].includes(assignee) || /^\d+$/.test(assignee), "assignee");
    const id = assignee === "me" ? user : assignee === "none" ? null : Number(assignee);
    tasks = tasks.filter((t) => t.assigneeId === id);
  }
  if (label) tasks = tasks.filter((t) => t.labels.includes(label.toLowerCase()));
  if (search) tasks = tasks.filter((t) => `${t.title} ${t.description}`.toLowerCase().includes(search.toLowerCase()));
  const sort = q.get("sort") ?? "created";
  valid(["created", "-created", "priority"].includes(sort), "sort");
  if (sort === "-created") tasks.reverse();
  if (sort === "priority") tasks.sort((a, b) => ["high", "normal", "low"].indexOf(a.priority) -
    ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number);
  let limit = q.get("limit") ?? "20";
  valid(/^\d+$/.test(limit) && Number(limit) >= 1 && Number(limit) <= 100, "limit");
  let offset = 0, cursor = q.get("cursor");
  if (cursor) {
    try { offset = Number(atob(cursor)); if (!Number.isInteger(offset) || offset < 0) throw 0; }
    catch { fail(400, "validation", "Invalid cursor", "cursor"); }
  }
  const items = tasks.slice(offset, offset + Number(limit)).map(view);
  const next = offset + items.length < tasks.length ? btoa(String(offset + items.length)) : null;
  return ok({ items, nextCursor: next });
}
function validate(c: Ctx, pid: number, b: any) {
  try {
    makeTask({
      ...c,
      s: {
        ...c.s,
        tasks: [],
        ids: { ...c.s.ids },
        nextTask: new Map(c.s.nextTask),
      },
    }, pid, b, c.user!);
  }
  catch (e: any) { throw e; }
}
