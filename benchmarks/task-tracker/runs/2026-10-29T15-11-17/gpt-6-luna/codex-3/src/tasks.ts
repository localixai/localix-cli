import type { State, Task } from "./model";
import { iso, memberRole, taskByKey, visibleProject } from "./model";
import { auth, bad, body, empty, fail, json } from "./http";
import { createTask, validateFields } from "./taskdata";

import { list, comments, dependencies } from "./taskextra";

const num = (v: string) => /^\d+$/.test(v) ? Number(v) : NaN;
function project(s: State, id: number, u: number) {
  const p = s.projects.find((x) => x.id === id);
  if (!p || !visibleProject(s, p, u)) fail(404, "not_found", "Project not found");
  return p;
}
function task(s: State, key: string, u: number) {
  const t = taskByKey(s, key);
  const p = t && s.projects.find((x) => x.id === t.projectId);
  if (!t || !p || !visibleProject(s, p, u)) fail(404, "not_found", "Task not found");
  return t;
}
function view(t: Task) { const { deleted, ...out } = t; return out; }
function etag(t: Task) { return { ETag: `"${t.version}"` }; }
export async function tasks(req: Request, path: string[], s: State): Promise<Response | undefined> {
  if (path[0] === "projects" && path[2] === "tasks") {
    const u = auth(req, s), p = project(s, num(path[1]!), u);
    if (path.length === 3 && req.method === "GET") return list(req, s, p.id, u);
    if ((path.length === 3 || (path.length === 4 && path[3] === "bulk")) && req.method === "POST") {
      if (memberRole(s, p, u) === "viewer") fail(403, "forbidden", "A viewer cannot create tasks");
      const b = await body(req);
      if (path[3] === "bulk") {
        if (!Array.isArray(b.tasks) || b.tasks.length < 1 || b.tasks.length > 50) bad("tasks");
        b.tasks.forEach((x: any, i: number) => validateFields(x, s, p.id, `tasks[${i}]`));
        return json({ items: b.tasks.map((x: any) => view(createTask(s, p, u, x))) }, 201);
      }
      const key = req.headers.get("Idempotency-Key");
      if (key) {
        const ik = `${u}:${key}`, prior = s.idem.get(ik);
        if (prior && s.now() - prior.at < 86400000) {
          if (prior.body !== JSON.stringify(b)) fail(422, "idempotency_mismatch", "Key used with another request");
          return json(prior.reply.body, prior.reply.status, { ...prior.reply.headers, "Idempotent-Replayed": "true" });
        }
        validateFields(b, s, p.id);
        const t = createTask(s, p, u, b);
        const reply = { body: view(t), status: 201, headers: etag(t) };
        s.idem.set(ik, { at: s.now(), body: JSON.stringify(b), reply });
        return json(reply.body, 201, reply.headers);
      }
      validateFields(b, s, p.id); const t = createTask(s, p, u, b);
      return json(view(t), 201, etag(t));
    }
  }
  if (path[0] !== "tasks") return undefined;
  const u = auth(req, s), key = path[1] ?? "", t = task(s, key, u);
  const p = s.projects.find((x) => x.id === t.projectId)!;
  if (path.length === 2 && req.method === "GET") return json(view(t), 200, etag(t));
  if (path.length === 2 && req.method === "PATCH") {
    if (memberRole(s, p, u) === "viewer") fail(403, "forbidden", "A viewer cannot edit tasks");
    const b = await body(req); validateFields(b, s, p.id, "", true);
    checkMatch(req, t);
    const order = ["assigneeId", "description", "labels", "priority", "title"];
    const old: any = { ...t, labels: [...t.labels] }; const next = { ...old, ...b };
    if ("title" in b) next.title = b.title.trim();
    if ("description" in b) next.description = b.description;
    if ("labels" in b) next.labels = cleanLabels(b.labels);
    const changed = order.filter((k) => JSON.stringify((old as any)[k]) !== JSON.stringify((next as any)[k]));
    if (changed.length) {
      for (const field of changed) {
        s.histories.get(t.id)!.push({
          at: iso(s), by: u, field, from: (old as any)[field], to: (next as any)[field],
        });
      }
      Object.assign(t, next); t.version++; t.updatedAt = iso(s);
    }
    return json(view(t), 200, etag(t));
  }
  if (path.length === 2 && req.method === "DELETE") {
    if (memberRole(s, p, u) !== "owner" && t.createdBy !== u) fail(403, "forbidden", "Cannot delete this task");
    t.deleted = true;
    for (const set of s.deps.values()) set.delete(t.id);
    s.deps.delete(t.id);
    return empty();
  }
  if (path[2] === "transitions" && req.method === "POST") {
    if (memberRole(s, p, u) === "viewer") fail(403, "forbidden", "A viewer cannot transition tasks");
    const b = await body(req);
    if (!["open", "in_progress", "done"].includes(b.to)) bad("to");
    checkMatch(req, t);
    const allowed: Record<string, string[]> = { open: ["in_progress"], in_progress: ["open", "done"], done: ["open"] };
    if (!allowed[t.status]!.includes(b.to)) fail(409, "invalid_transition", "Invalid transition");
    if (b.to === "done") {
      const deps = [...(s.deps.get(t.id) ?? [])].map((id) => s.tasks.find((x) => x.id === id));
      if (deps.some((x) => x && !x.deleted && x.status !== "done")) fail(409, "blocked", "A dependency is not done");
    }
    s.histories.get(t.id)!.push({ at: iso(s), by: u, field: "status", from: t.status, to: b.to });
    t.status = b.to; t.version++; t.updatedAt = iso(s);
    return json(view(t), 200, etag(t));
  }
  if (path[2] === "history" && req.method === "GET") return json(s.histories.get(t.id) ?? []);
  if (path[2] === "comments") return comments(req, s, t, u, path[3]);
  if (path[2] === "dependencies") {
    if (req.method !== "GET" && memberRole(s, p, u) === "viewer") {
      fail(403, "forbidden", "A viewer cannot change dependencies");
    }
    return dependencies(req, s, t, u, path[3]);
  }
  return undefined;
}
function checkMatch(req: Request, t: Task) {
  const match = req.headers.get("If-Match");
  if (match && match !== `"${t.version}"`) fail(412, "precondition_failed", "Task version changed");
}
function cleanLabels(a: string[]) { return [...new Set(a.map((x) => x.trim().toLowerCase()))].sort(); }
