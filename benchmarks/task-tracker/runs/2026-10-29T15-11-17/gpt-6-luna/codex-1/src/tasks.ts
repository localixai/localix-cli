import { dto, make, etag, checkMatch, log } from "./task-utils";
import { dependency } from "./dependencies";
import {
  body,
  fail,
  iso,
  json,
  projectFor,
  requireRole,
  validate,
  type State,
  type Task,
} from "./model";
export async function tasks(req: Request, path: string, s: State, u: number) {
  let m = path.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if (m && req.method === "POST") {
    const p = projectFor(s, Number(m[1]), u);
    requireRole(p, u);
    const b = await body(req);
    if (m[2]) {
      validate(
        Array.isArray(b.tasks) && b.tasks.length >= 1 && b.tasks.length <= 50,
        "tasks",
      );
      const seq = p.seq,
        id = s.counters.task;
      try {
        const items = b.tasks.map((v: any, i: number) =>
          make(v, p, u, s, (f) => `tasks[${i}].${f}`),
        );
        s.tasks.push(...items);
        return json({ items: items.map(dto) }, 201);
      } catch (e) {
        p.seq = seq;
        s.counters.task = id;
        throw e;
      }
    }
    const idem = req.headers.get("Idempotency-Key");
    if (idem) {
      const k = `${u}:${idem}`,
        old = s.idem.get(k);
      if (old && s.now() - old.at < 86400000) {
        if (JSON.stringify(old.body) !== JSON.stringify(b))
          fail(422, "idempotency_mismatch");
        return json(old.result, 201, {
          ETag: '"1"',
          "Idempotent-Replayed": "true",
        });
      }
    }
    const t = make(b, p, u, s);
    s.tasks.push(t);
    const result = dto(t);
    if (idem) s.idem.set(`${u}:${idem}`, { at: s.now(), body: b, result });
    return json(result, 201, etag(t));
  }
  m = path.match(
    /^\/tasks\/([^/]+)(?:\/(transitions|history|comments|dependencies))?(?:\/([^/]+))?$/,
  );
  if (!m) return undefined;
  const key = m[1],
    sub = m[2],
    tail = m[3],
    t = s.tasks.find((x) => x.key === key);
  if (!t) fail(404, "not_found");
  const p = projectFor(s, t.projectId, u);
  if (!sub && req.method === "GET") return json(dto(t), 200, etag(t));
  if (sub === "history" && req.method === "GET") return json(t.history);
  if (sub === "comments") {
    if (req.method === "GET") return json(t.comments);
    if (req.method === "POST") {
      const b = await body(req),
        text = typeof b.body === "string" ? b.body.trim() : "";
      validate(text.length > 0 && text.length <= 2000, "body");
      const c = {
        id: ++s.counters.comment,
        authorId: u,
        body: text,
        createdAt: iso(s),
      };
      t.comments.push(c);
      return json(c, 201);
    }
  }
  if (sub === "dependencies") return dependency(req, t, tail, s, u);
  if (sub === "transitions" && req.method === "POST") {
    requireRole(p, u);
    checkMatch(req, t);
    const b = await body(req);
    validate(["open", "in_progress", "done"].includes(b.to), "to");
    const allowed =
      (t.status === "open" && b.to === "in_progress") ||
      (t.status === "in_progress" && ["open", "done"].includes(b.to)) ||
      (t.status === "done" && b.to === "open");
    if (!allowed) fail(409, "invalid_transition");
    if (
      b.to === "done" &&
      [...t.deps].some(
        (k) => s.tasks.find((x) => x.key === k)?.status !== "done",
      )
    )
      fail(409, "blocked");
    const from = t.status;
    t.status = b.to;
    t.version++;
    t.updatedAt = iso(s);
    log(t, u, s, "status", from, t.status);
    return json(dto(t), 200, etag(t));
  }
  if (!sub && req.method === "PATCH") {
    requireRole(p, u);
    checkMatch(req, t);
    const b = await body(req),
      allowed = ["title", "description", "priority", "labels", "assigneeId"];
    for (const k of Object.keys(b)) validate(allowed.includes(k), k);
    const old = {
      title: t.title,
      description: t.description,
      priority: t.priority,
      labels: t.labels,
      assigneeId: t.assigneeId,
    };
    const clean: any = { ...b };
    if ("title" in clean) {
      validate(
        typeof clean.title === "string" &&
          clean.title.trim().length > 0 &&
          clean.title.trim().length <= 200,
        "title",
      );
      clean.title = clean.title.trim();
    }
    if ("description" in clean)
      validate(typeof clean.description === "string", "description");
    if ("priority" in clean)
      validate(["low", "normal", "high"].includes(clean.priority), "priority");
    if ("labels" in clean)
      validate(
        Array.isArray(clean.labels) &&
          clean.labels.every(
            (x: any) =>
              typeof x === "string" &&
              !/\s/.test(x.trim()) &&
              x.trim().length <= 30,
          ),
        "labels",
      );
    if ("labels" in clean)
      clean.labels = [
        ...new Set(clean.labels.map((x: string) => x.trim().toLowerCase())),
      ].sort();
    if ("assigneeId" in clean)
      validate(
        clean.assigneeId === null || p.members.has(clean.assigneeId),
        "assigneeId",
      );
    let changed = false;
    for (const k of Object.keys(clean).sort()) {
      const from = old[k as keyof typeof old],
        to = clean[k];
      if (JSON.stringify(from) !== JSON.stringify(to)) {
        (t as any)[k] = to;
        log(t, u, s, k, from, to);
        changed = true;
      }
    }
    if (changed) {
      t.version++;
      t.updatedAt = iso(s);
    }
    return json(dto(t), 200, etag(t));
  }
  if (!sub && req.method === "DELETE") {
    requireRole(p, u);
    if (u !== p.ownerId && t.createdBy !== u) fail(403, "forbidden");
    s.tasks = s.tasks.filter((x) => x !== t);
    for (const x of s.tasks) x.deps.delete(t.key);
    return new Response(null, { status: 204 });
  }
  return undefined;
}
