import { json, fail, bad, notFound, forbidden, stamp } from "./http";
import { member } from "./projects";

export function tasksRoute(c: any): Response {
  const { db, path, method, body: b, uid, url, req } = c;
  if (!uid) return fail(401, "unauthorized", "Unauthorized");
  const pm = path.match(/^\/projects\/(\d+)\/(tasks|stats)(?:\/(bulk))?$/);
  if (pm) {
    const p = db.projects.find((x: any) => x.id === +pm[1] && member(x, uid));
    if (!p) return notFound();
    if (pm[2] === "stats" && method === "GET") return stats(db, p);
    if (pm[2] === "tasks" && method === "GET") return listing(c, p);
    if (pm[2] === "tasks" && method === "POST") {
      if (!writer(p, uid)) return forbidden();
      const bulk = !!pm[3], list = bulk ? b.tasks : [b];
      if (bulk && (!Array.isArray(list) || list.length < 1 || list.length > 50)) return bad("tasks");
      const made = [];
      for (let i = 0; i < list.length; i++) {
        const err = validate(list[i], p);
        if (err) return bad(bulk ? `tasks[${i}].${err}` : err);
      }
      const idem = req.headers.get("Idempotency-Key");
      const prior = !bulk && idem && p.idem?.[`${uid}:${idem}`];
      if (prior && prior.expires > db.now()) {
        if (prior.body !== JSON.stringify(b)) return fail(422, "idempotency_mismatch", "Key reused");
        return json(publicTask(prior.task), 201,
          { ETag: `"${prior.task.version}"`, "Idempotent-Replayed": "true" });
      }
      for (const x of list) made.push(create(db, p, uid, x));
      if (!bulk && idem) (p.idem ??= {})[`${uid}:${idem}`] =
        { body: JSON.stringify(b), task: made[0], expires: db.now() + 86400000 };
      return json(bulk ? { items: made.map(publicTask) } : publicTask(made[0]), 201,
        !bulk && made.length ? { ETag: '"1"' } : {});
    }
  }
  const m = path.match(/^\/tasks\/([^/]+)(?:\/(.*))?$/);
  if (!m) return notFound();
  const t = db.tasks.find((x: any) => x.key === m[1] && !x.deleted);
  if (!t) return notFound();
  const p = db.projects.find((x: any) => x.id === t.projectId && member(x, uid));
  if (!p) return notFound();
  const sub = m[2] || "";
  if (!sub && method === "GET") return json(publicTask(t), 200, { ETag: `"${t.version}"` });
  if (!writer(p, uid) && ["PATCH", "DELETE", "POST"].includes(method) &&
      sub !== "comments" && !sub.startsWith("dependencies")) return forbidden();
  if (sub === "history" && method === "GET") return json(t.history);
  if (sub === "comments" && method === "GET") return json(t.comments);
  if (sub === "comments" && method === "POST") {
    const text = b.body;
    if (typeof text !== "string" || !text.trim() || text.length > 2000) return bad("body");
    const item = { id: ++db.counters.comment, authorId: uid, body: text, createdAt: stamp(db) };
    t.comments.push(item); return json(item, 201);
  }
  if (sub.startsWith("dependencies")) return dependencies(c, t, p);
  if (sub === "transitions" && method === "POST") return transition(c, t);
  if (!sub && method === "PATCH") return update(c, t, p);
  if (!sub && method === "DELETE") {
    if (uid !== p.ownerId && t.createdBy !== uid) return forbidden();
    t.deleted = true;
    db.tasks.forEach((x: any) => { x.dependsOn = x.dependsOn.filter((k: string) => k !== t.key); });
    return json(null, 204);
  }
  return notFound();
}
const writer = (p: any, id: number) => p.members.some((x: any) => x.userId === id && x.role !== "viewer");
function validate(b: any, p: any): string | null {
  if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200) return "title";
  if (b.description !== undefined && typeof b.description !== "string") return "description";
  if (b.priority !== undefined && !["low", "normal", "high"].includes(b.priority)) return "priority";
  if (b.labels !== undefined && (!Array.isArray(b.labels) ||
    b.labels.some((x: any) => typeof x !== "string" || !x.trim() || x.trim().length > 30 || /\s/.test(x.trim()))))
    return "labels";
  if (b.assigneeId != null && !p.members.some((x: any) => x.userId === b.assigneeId)) return "assigneeId";
  return null;
}
function create(db: any, p: any, uid: number, b: any): any {
  const n = (p.next ??= 0) + 1; p.next = n;
  const at = stamp(db), labels = [...new Set((b.labels || []).map((x: string) => x.trim().toLowerCase()))].sort();
  const t = { id: ++db.counters.task, key: `${p.key}-${n}`, number: n, projectId: p.id,
    title: b.title.trim(), description: b.description ?? "", status: "open", priority: b.priority ?? "normal",
    labels, assigneeId: b.assigneeId ?? null, createdBy: uid, createdAt: at, updatedAt: at, version: 1,
    history: [], comments: [], dependsOn: [], deleted: false };
  db.tasks.push(t); return t;
}
function listing(c: any, p: any): Response {
  let a = c.db.tasks.filter((t: any) => t.projectId === p.id && !t.deleted);
  const q = c.url.searchParams;
  if (q.has("status")) a = a.filter((t: any) => t.status === q.get("status"));
  if (q.has("label")) a = a.filter((t: any) => t.labels.includes(q.get("label")!.toLowerCase()));
  if (q.has("q")) {
    const term = q.get("q")!.toLowerCase();
    a = a.filter((t: any) => `${t.title} ${t.description}`.toLowerCase().includes(term));
  }
  if (q.has("assignee")) {
    const v = q.get("assignee");
    a = a.filter((t: any) => v === "none" ? !t.assigneeId : v === "me" ? t.assigneeId === c.uid : t.assigneeId === +v!);
  }
  if (q.get("sort") === "-created") a.reverse();
  if (q.get("sort") === "priority") {
    const order: any = { high: 0, normal: 1, low: 2 };
    a.sort((x: any, y: any) => order[x.priority] - order[y.priority] || x.number - y.number);
  }
  const limitText = q.get("limit"), cursorText = q.get("cursor");
  if (q.has("status") && !["open", "in_progress", "done"].includes(q.get("status")!)) return bad("status");
  if (q.has("sort") && !["created", "-created", "priority"].includes(q.get("sort")!)) return bad("sort");
  if (limitText && (!/^\d+$/.test(limitText) ||
      +limitText < 1 || +limitText > 100)) return bad("limit");
  if (cursorText && !/^\d+$/.test(cursorText)) return bad("cursor");
  if (q.has("assignee") && !["me", "none"].includes(q.get("assignee")!) &&
      !/^\d+$/.test(q.get("assignee")!)) return bad("assignee");
  const limit = +(limitText || 20);
  const cursor = +(q.get("cursor") || 0), items = a.slice(cursor, cursor + limit);
  return json({ items: items.map(publicTask), nextCursor: cursor + limit < a.length ? String(cursor + limit) : null });
}
const publicTask = (t: any) => {
  const { history, comments, dependsOn, deleted, ...out } = t;
  return out;
};
function stats(db: any, p: any): Response {
  const a = db.tasks.filter((t: any) => t.projectId === p.id && !t.deleted);
  const counts = (field: string, values: string[]) =>
    Object.fromEntries(values.map((v) => [v, a.filter((t: any) => t[field] === v).length]));
  const byAssignee = new Map<number, number>();
  a.forEach((t: any) => { if (t.assigneeId) byAssignee.set(t.assigneeId, (byAssignee.get(t.assigneeId) || 0) + 1); });
  return json({ total: a.length, byStatus: counts("status", ["open", "in_progress", "done"]),
    byPriority: counts("priority", ["high", "normal", "low"]),
    byAssignee: [...byAssignee].map(([userId, count]) => ({ userId, count }))
      .sort((x, y) => y.count - x.count || x.userId - y.userId) });
}
function update(c: any, t: any, p: any): Response {
  const { body: b, db, uid, req } = c;
  const fields = Object.keys(b);
  for (const f of fields) if (!["title", "description", "priority", "labels", "assigneeId"].includes(f)) return bad(f);
  const err = validate({ ...t, ...b, title: b.title ?? t.title }, p);
  if (err) return bad(err);
  const match = req.headers.get("If-Match");
  if (match && match !== `"${t.version}"`) return fail(412, "precondition_failed", "Version changed");
  const changes: any[] = [];
  for (const f of fields.sort()) {
    const value = f === "labels" ? [...new Set(b[f].map((x: string) => x.trim().toLowerCase()))].sort()
      : f === "title" ? b[f].trim() : b[f];
    if (JSON.stringify(t[f]) !== JSON.stringify(value)) {
      changes.push({ at: stamp(db), by: uid, field: f, from: t[f], to: value }); t[f] = value;
    }
  }
  if (changes.length) {
    t.history.push(...changes); t.version++; t.updatedAt = stamp(db);
  }
  return json(publicTask(t), 200, { ETag: `"${t.version}"` });
}
function transition(c: any, t: any): Response {
  const { body: b, db, uid, req } = c, to = b.to;
  if (!["open", "in_progress", "done"].includes(to)) return bad("to");
  const match = req.headers.get("If-Match");
  if (match && match !== `"${t.version}"`) return fail(412, "precondition_failed", "Version changed");
  const allowed: any = { open: ["in_progress"], in_progress: ["open", "done"], done: ["open"] };
  if (!allowed[t.status].includes(to)) return fail(409, "invalid_transition", "Invalid transition");
  if (!writer(db.projects.find((p: any) => p.id === t.projectId), uid)) return forbidden();
  if (to === "done" && t.dependsOn.some((key: string) =>
    db.tasks.some((x: any) => x.key === key && x.status !== "done" && !x.deleted)))
    return fail(409, "blocked", "Dependencies are not done");
  const from = t.status, at = stamp(db);
  t.status = to; t.version++; t.updatedAt = at;
  t.history.push({ at, by: uid, field: "status", from, to });
  return json(t, 200, { ETag: `"${t.version}"` });
}
function dependencies(c: any, t: any, p: any): Response {
  const { db, method, body: b, path, uid } = c;
  const blocks = () => db.tasks.filter((x: any) => !x.deleted && x.dependsOn.includes(t.key))
    .sort((x: any, y: any) => x.number - y.number).map((x: any) => x.key);
  const view = () => ({ key: t.key, dependsOn: [...t.dependsOn].sort((a, b) =>
    Number(a.split("-").pop()) - Number(b.split("-").pop())), blocks: blocks() });
  if (method === "GET") return json(view());
  if (!writer(p, uid)) return forbidden();
  const remove = path.match(/\/dependencies\/([^/]+)$/);
  if (remove && method === "DELETE") {
    const i = t.dependsOn.indexOf(remove[1]);
    if (i < 0) return notFound();
    t.dependsOn.splice(i, 1); return json(null, 204);
  }
  if (method !== "POST") return notFound();
  const other = db.tasks.find((x: any) => x.key === b.dependsOn && !x.deleted);
  if (!other || other.projectId !== p.id || other.id === t.id) return bad("dependsOn");
  if (t.dependsOn.includes(other.key)) return fail(409, "conflict", "Already depends");
  const reaches = (node: any, target: string, seen = new Set<string>()): boolean => {
    if (node.key === target) return true;
    if (seen.has(node.key)) return false;
    seen.add(node.key);
    return node.dependsOn.some((k: string) => {
      const next = db.tasks.find((x: any) => x.key === k && !x.deleted);
      return next && reaches(next, target, seen);
    });
  };
  if (reaches(other, t.key)) return fail(409, "dependency_cycle", "Dependency cycle");
  t.dependsOn.push(other.key); return json(view(), 201);
}
