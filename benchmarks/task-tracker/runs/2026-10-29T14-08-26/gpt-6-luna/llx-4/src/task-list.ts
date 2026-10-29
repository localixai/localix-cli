import type { State, Project, Task } from "./types";
import { fail, writer, makeTask, taskJson, valid } from "./util";
const json = (x: unknown, status = 200, headers: Record<string, string> = {}) =>
  Response.json(x, { status, headers });
export function projectTasks(s: State, method: string, path: string, p: Project,
  uid: number, body: any, url: URL, headers: Headers): Response {
  if (path.endsWith("/bulk")) {
    writer(p, uid);
    if (method !== "POST") fail(404, "not_found");
    const list = body.tasks;
    valid(Array.isArray(list) && list.length >= 1 && list.length <= 50, "tasks");
    const start = p.next;
    try { return json({ items: list.map((x: unknown, i: number) => {
      try { return taskJson(makeTask(s, p, uid, x)); }
      catch (e: any) { if (e.field) e.field = `tasks[${i}].${e.field}`; throw e; }
    }) }, 201); } catch (e) {
      s.tasks = s.tasks.filter((t) => t.projectId !== p.id || t.number < start);
      p.next = start;
      throw e;
    }
  }
  if (method === "POST") {
    writer(p, uid);
    const idempotencyKey = headers.get("Idempotency-Key");
    const storageKey = idempotencyKey ? `${uid}:${idempotencyKey}` : "";
    const saved = storageKey ? s.idem.get(storageKey) : undefined;
    if (saved && s.now() - saved.time < 86400000) {
      if (saved.body !== JSON.stringify(body)) fail(422, "idempotency_mismatch");
      return json(saved.result, 201, { "Idempotent-Replayed": "true", ETag: '"1"' });
    }
    const t = makeTask(s, p, uid, body);
    const result = taskJson(t);
    if (storageKey) s.idem.set(storageKey, { time: s.now(), body: JSON.stringify(body), result });
    return json(result, 201, { ETag: '"1"' });
  }
  let list = s.tasks.filter((t) => t.projectId === p.id);
  const q = url.searchParams;
  if (q.has("status")) {
    valid(["open", "in_progress", "done"].includes(q.get("status") ?? ""), "status");
    list = list.filter((t) => t.status === q.get("status"));
  }
  if (q.has("assignee")) {
    const a = q.get("assignee")!;
    valid(a === "me" || a === "none" || /^\d+$/.test(a), "assignee");
    list = list.filter((t) => a === "none" ? t.assigneeId === null :
      t.assigneeId === (a === "me" ? uid : Number(a)));
  }
  if (q.has("label")) list = list.filter((t) => t.labels.includes(q.get("label")!.toLowerCase()));
  if (q.has("q")) {
    const term = q.get("q")!.toLowerCase();
    list = list.filter((t) => `${t.title} ${t.description}`.toLowerCase().includes(term));
  }
  const sort = q.get("sort") ?? "number";
  valid(["number", "-created", "priority"].includes(sort), "sort");
  const rank: Record<string, number> = { high: 0, normal: 1, low: 2 };
  if (sort === "-created") list.sort((a, b) => b.number - a.number);
  else if (sort === "priority") list.sort((a, b) => rank[a.priority]! - rank[b.priority]! || a.number - b.number);
  else list.sort((a, b) => a.number - b.number);
  const limitRaw = q.get("limit") ?? "20", limit = Number(limitRaw);
  valid(/^\d+$/.test(limitRaw) && limit >= 1 && limit <= 100, "limit");
  if (q.has("cursor")) {
    let cursor: any;
    try { cursor = JSON.parse(atob(q.get("cursor") ?? "")); }
    catch { return fail(400, "validation", "Invalid cursor", "cursor"); }
    valid(cursor.sort === sort && cursor.offset >= 0, "cursor");
    list = list.slice(cursor.offset);
  }
  const offset = q.has("cursor") ? JSON.parse(atob(q.get("cursor")!)).offset : 0;
  const items = list.slice(0, limit).map(taskJson);
  const nextCursor = offset + items.length < list.length
    ? btoa(JSON.stringify({ sort, offset: offset + items.length })) : null;
  return json({ items, nextCursor });
}
