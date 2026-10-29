import { dependencies } from "./dependencies";
import { list } from "./listing";
import { make, output } from "./task-data";
import { patch, transition } from "./task-operations";
import {
  body,
  err,
  iso,
  json,
  projectFor,
  role,
  taskFor,
  validation,
  type Ctx,
  type Task,
} from "./model";
export async function tasks(
  c: Ctx,
  path: string,
  method: string,
): Promise<Response | null> {
  const pm = path.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if (pm) {
    const p = projectFor(c, Number(pm[1]));
    if (!p) return err("not_found", 404);
    if (method === "POST" && role(c, p) === "viewer")
      return err("forbidden", 403);
    if (method === "POST" && !pm[2]) {
      const b = await body(c);
      if (!b) return validation();
      const ik = c.req.headers.get("Idempotency-Key"),
        key = ik ? `${c.user!.id}:${ik}` : "",
        old = key ? c.db.idem.get(key) : null;
      if (old && c.now() - old.at < 86400000) {
        if (old.hash !== JSON.stringify(b))
          return err("idempotency_mismatch", 422);
        return json(old.body, 201, {
          ETag: `"${old.body.version}"`,
          "Idempotent-Replayed": "true",
        });
      }
      const t = make(c, p, b);
      if (t instanceof Response) return t;
      const out = output(t);
      if (key)
        c.db.idem.set(key, { at: c.now(), hash: JSON.stringify(b), body: out });
      return json(out, 201, { ETag: '"1"' });
    }
    if (method === "POST" && pm[2]) {
      const b = await body(c);
      if (
        !b ||
        !Array.isArray(b.tasks) ||
        b.tasks.length < 1 ||
        b.tasks.length > 50
      )
        return validation("tasks");
      const made: Task[] = [];
      const taskCount = c.db.tasks.length;
      const nextNumber = p.next;
      for (let i = 0; i < b.tasks.length; i++) {
        const t = make(c, p, b.tasks[i]);
        if (t instanceof Response) {
          const e: any = await t.json();
          c.db.tasks.length = taskCount;
          p.next = nextNumber;
          return validation(`tasks[${i}].${e.error.field || "title"}`);
        }
        made.push(t);
      }
      return json({ items: made.map(output) }, 201);
    }
    if (method === "GET" && !pm[2]) return list(c, p.id);
  }
  const m = path.match(
    /^\/tasks\/([^/]+)(?:\/(comments|history|dependencies(?:\/(?:[^/]+))?|transitions))?$/,
  );
  if (!m) return null;
  const t = taskFor(c, m[1]!);
  if (!t) return err("not_found", 404);
  const suffix = m[2],
    p = c.db.projects.find((x) => x.id === t.projectId)!;
  if (suffix === "comments") {
    if (method === "GET") return json(t.comments);
    if (method === "POST") {
      const b = await body(c);
      if (!b) return validation();
      if (typeof b.body !== "string" || !b.body.trim() || b.body.length > 2000)
        return validation("body");
      const item = {
        id: t.comments.length + 1,
        authorId: c.user!.id,
        body: b.body,
        createdAt: iso(c),
      };
      t.comments.push(item);
      return json(item, 201);
    }
  }
  if (suffix === "history" && method === "GET") return json(t.history);
  if (suffix === "transitions" && method === "POST") return transition(c, t);
  if (suffix?.startsWith("dependencies"))
    return dependencies(c, t, path, method);
  if (!suffix && method === "GET")
    return json(output(t), 200, { ETag: `"${t.version}"` });
  if (!suffix && method === "PATCH") return patch(c, t);
  if (!suffix && method === "DELETE") {
    if (role(c, p) === "viewer") return err("forbidden", 403);
    if (t.createdBy !== c.user!.id && c.user!.id !== p.ownerId)
      return err("forbidden", 403);
    t.deleted = true;
    c.db.tasks.forEach(
      (x) => (x.depends = x.depends.filter((id) => id !== t.id)),
    );
    return new Response(null, { status: 204 });
  }
  return null;
}
