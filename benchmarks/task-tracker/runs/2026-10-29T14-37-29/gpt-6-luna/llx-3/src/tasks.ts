import type { State } from "./state";
import { bad, body, err, json, member, record, taskByKey, taskReply } from "./util";
import { makeTask, permission, validateTask } from "./task-core";
import { taskExtras } from "./task-extras";

export async function tasksRoute(req: Request, s: State, u: any, url: URL) {
  const path = url.pathname, projectMatch = path.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if (projectMatch && req.method === "POST") {
    const p = s.projects.find(x => x.id === Number(projectMatch[1]));
    if (!member(p, u.id)) return err(404, "not_found", "Project not found");
    if (!["owner", "member"].includes(permission(p, u))) return err(403, "forbidden", "Cannot create tasks");
    const b = await body(req); if (!b) return err(400, "validation", "Invalid JSON");
    if (projectMatch[2]) {
      if (!Array.isArray(b.tasks) || !b.tasks.length || b.tasks.length > 50) return bad("tasks");
      for (let i = 0; i < b.tasks.length; i++) {
        const f = validateTask(b.tasks[i], s, p);
        if (f) return bad(`tasks[${i}].${f}`);
      }
      return json({ items: b.tasks.map((x: any) => makeTask(s, p, u, x)) }, 201);
    }
    const idemKey = req.headers.get("Idempotency-Key");
    const key = idemKey ? `${u.id}:${idemKey}` : "";
    const old = key && s.idem.get(key);
    if (old && s.now() - old.time < 86400000) {
      if (old.body !== JSON.stringify(b)) return err(422, "idempotency_mismatch", "Key used with another body");
      return json(old.result, 201, { "Idempotent-Replayed": "true", ETag: '"1"' });
    }
    const field = validateTask(b, s, p); if (field) return bad(field);
    const t = makeTask(s, p, u, b);
    if (key) s.idem.set(key, { time: s.now(), body: JSON.stringify(b), result: t });
    return json(t, 201, { ETag: `"${t.version}"` });
  }
  const mt = path.match(/^\/tasks\/([^/]+)(?:\/(.*))?$/);
  if (!mt) return null;
  const t = taskByKey(s, decodeURIComponent(mt[1]!));
  if (!t) return err(404, "not_found", "Task not found");
  const p = s.projects.find(x => x.id === t.projectId);
  if (!member(p, u.id)) return err(404, "not_found", "Task not found");
  return taskExtras(req, s, u, t, p, mt[2] ?? "");
}
