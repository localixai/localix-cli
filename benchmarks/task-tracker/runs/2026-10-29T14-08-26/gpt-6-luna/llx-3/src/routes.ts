// @ts-nocheck
import { publicProject as pp, publicTask as pt, publicUser as pu, type Project, type State, type Task } from "./model";
import { bodyOf, fail, response, validate } from "./http";
import { type Ctx, stamp, user, proj, task, owner, editor, validTask, createTask,
  change, etag, match } from "./task-helpers";
import { dependencies } from "./dependencies";
const allowed: Record<string, string[]> = {
  "/users": ["POST"], "/sessions": ["POST", "DELETE"], "/me": ["GET"], "/projects": ["GET", "POST"],
};
export async function route(c: Ctx): Promise<Response> {
  const { req, url, s } = c, method = req.method, path = url.pathname;
  const id = (x: string) => /^\d+$/.test(x) ? Number(x) : 0;
  const parts = path.split("/").filter(Boolean);
  const known = path === "/users" || path === "/sessions" || path === "/me" ||
    path === "/projects" || path.startsWith("/projects/") || path.startsWith("/tasks/");
  if (!known) fail(404, "not_found", "Not found");
  const base = path === "/projects" ? allowed[path] :
    path === "/projects/" + parts[1] ? ["GET"] : undefined;
  if (base && !base.includes(method)) return response({ error: { code: "method_not_allowed",
    message: "Method not allowed" } }, 405, { Allow: base.join(", ") });
  if (path === "/users" && method === "POST") {
    const b = await bodyOf(req), email = typeof b.email === "string" ? b.email.trim().toLowerCase() : "";
    validate(/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email), "email");
    validate(typeof b.password === "string" && b.password.length >= 8, "password");
    validate(typeof b.name === "string" && b.name.trim().length > 0 && b.name.trim().length <= 100, "name");
    if (s.users.some(u => u.email === email)) fail(409, "conflict", "Email already exists", "email");
    const u = { id: ++s.uid, email, password: b.password, name: b.name.trim() };
    s.users.push(u); return response(pu(u), 201);
  }
  if (path === "/sessions" && method === "POST") {
    const b = await bodyOf(req), u = s.users.find(z => z.email === String(b.email).toLowerCase()
      && z.password === b.password);
    if (!u) fail(401, "unauthorized", "Invalid credentials");
    const token = crypto.randomUUID(); s.sessions.set(token, u.id);
    return response({ token }, 201);
  }
  if (path === "/sessions" && method === "DELETE") {
    const u = user(c); void u; if (c.token) s.sessions.delete(c.token);
    return response(null, 204);
  }
  if (path === "/me" && method === "GET") return response(pu(user(c)));
  if (path === "/projects" && method === "GET") {
    const u = user(c);
    return response(s.projects.filter(p => p.members.has(u.id)).map(pp));
  }
  if (path === "/projects" && method === "POST") {
    const u = user(c), b = await bodyOf(req);
    validate(typeof b.name === "string" && !!b.name.trim() && b.name.trim().length <= 100, "name");
    validate(typeof b.key === "string" && /^[A-Z]{2,6}$/.test(b.key), "key");
    if (s.projects.some(p => p.key === b.key)) fail(409, "conflict", "Key already exists", "key");
    const p: Project = { id: ++s.pid, name: b.name.trim(), key: b.key, ownerId: u.id,
      createdAt: stamp(c), next: 1, members: new Map([[u.id, "owner"]]) };
    s.projects.push(p); return response(pp(p), 201);
  }
  if (parts[0] === "projects" && parts[1] && parts.length === 2 && method === "GET") {
    return response(pp(proj(c, id(parts[1]))));
  }
  if (parts[0] === "projects" && parts[1] && parts[2] === "members") {
    const p = proj(c, id(parts[1])), u = user(c);
    if (parts.length === 3 && method === "GET")
      return response([...p.members].sort((a, b) => a[0] - b[0])
        .map(([userId, role]) => ({ userId, role })));
    if (parts.length === 3 && method === "POST") {
      if (!owner(p, u.id)) fail(403, "forbidden", "Owner only");
      const b = await bodyOf(req);
      validate(Number.isInteger(b.userId) && s.users.some(z => z.id === b.userId), "userId");
      validate(["member", "viewer"].includes(b.role), "role");
      if (p.members.has(b.userId)) fail(409, "conflict", "Already a member");
      p.members.set(b.userId, b.role); return response({ userId: b.userId, role: b.role }, 201);
    }
    if (parts.length === 4 && method === "DELETE") {
      if (!owner(p, u.id)) fail(403, "forbidden", "Owner only");
      const uid = id(parts[3]); if (!p.members.has(uid) || uid === p.ownerId) fail(404, "not_found", "Not found");
      p.members.delete(uid);
      for (const t of s.tasks) if (t.projectId === p.id && t.assigneeId === uid) t.assigneeId = null;
      return response(null, 204);
    }
  }
  if (parts[0] === "projects" && parts[1] && parts[2] === "stats" && method === "GET") {
    const p = proj(c, id(parts[1])), ts = s.tasks.filter(t => t.projectId === p.id);
    const count = (field: string, values: string[]) => Object.fromEntries(values.map(v =>
      [v, ts.filter(t => (t as any)[field] === v).length]));
    const assignees = new Map<number, number>();
    ts.forEach(t => { if (t.assigneeId) assignees.set(t.assigneeId, (assignees.get(t.assigneeId) || 0) + 1); });
    return response({ total: ts.length, byStatus: count("status", ["open", "in_progress", "done"]),
      byPriority: count("priority", ["high", "normal", "low"]),
      byAssignee: [...assignees].map(([userId, count]) => ({ userId, count }))
        .sort((a, b) => b.count - a.count || a.userId - b.userId) });
  }
  if (parts[0] === "projects" && parts[1] && parts[2] === "tasks") {
    const p = proj(c, id(parts[1])), u = user(c);
    if (parts.length === 3 && method === "POST") {
      if (!editor(p, u.id)) fail(403, "forbidden", "Read only");
      const b = await bodyOf(req), idem = req.headers.get("Idempotency-Key");
      const key = idem && `${u.id}:${idem}`, prior = key && s.idem.get(key);
      if (prior && c.now() - prior.at < 86400000) {
        if (prior.hash !== JSON.stringify(b)) fail(422, "idempotency_mismatch", "Key reused");
        return response(prior.task, 201, { "Idempotent-Replayed": "true", ETag: `"${prior.task.version}"` });
      }
      const t = createTask(c, p, b), out = pt(t);
      if (key) s.idem.set(key, { at: c.now(), hash: JSON.stringify(b), task: out });
      return response(out, 201, etag(t));
    }
    if (parts.length === 4 && parts[3] === "bulk" && method === "POST") {
      if (!editor(p, u.id)) fail(403, "forbidden", "Read only");
      const b = await bodyOf(req);
      validate(Array.isArray(b.tasks) && b.tasks.length >= 1 && b.tasks.length <= 50, "tasks");
      const values = b.tasks.map((x: any, i: number) => {
        try { validTask(x, p, c); return x; }
        catch (e: any) { if (e.field) e.field = `tasks[${i}].${e.field}`; throw e; }
      });
      return response({ items: values.map((x: any) => pt(createTask(c, p, x))) }, 201);
    }
    if (parts.length === 3 && method === "GET") {
      let ts = s.tasks.filter(t => t.projectId === p.id);
      const q = url.searchParams;
      const filters = ["status", "assignee", "label", "q", "sort", "limit", "cursor"];
      for (const k of q.keys()) validate(filters.includes(k), k);
      if (q.has("status")) { validate(["open", "in_progress", "done"].includes(q.get("status")!), "status");
        ts = ts.filter(t => t.status === q.get("status")); }
      if (q.has("assignee")) {
        const a = q.get("assignee")!;
        validate(a === "me" || a === "none" || /^\d+$/.test(a), "assignee");
        const target = a === "me" ? u.id : a === "none" ? null : Number(a);
        ts = ts.filter(t => t.assigneeId === target);
      }
      if (q.has("label")) ts = ts.filter(t => t.labels.includes(q.get("label")!.toLowerCase()));
      if (q.has("q")) { const v = q.get("q")!.toLowerCase();
        ts = ts.filter(t => `${t.title} ${t.description}`.toLowerCase().includes(v)); }
      const sort = q.get("sort") || "created";
      validate(["created", "-created", "priority"].includes(sort), "sort");
      ts.sort(sort === "-created" ? (a, b) => b.number - a.number :
        sort === "priority" ? (a, b) => ["high", "normal", "low"].indexOf(a.priority) -
          ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number :
          (a, b) => a.number - b.number);
      const limitText = q.get("limit") || "20", limit = Number(limitText);
      validate(/^\d+$/.test(limitText) && limit >= 1 && limit <= 100, "limit");
      let offset = 0;
      if (q.has("cursor")) {
        try { offset = Number(atob(q.get("cursor")!)); } catch { fail(400, "validation", "Invalid cursor", "cursor"); }
        validate(Number.isInteger(offset) && offset >= 0 && offset <= ts.length, "cursor");
      }
      const page = ts.slice(offset, offset + limit);
      const next = offset + limit < ts.length ? btoa(String(offset + limit)) : null;
      return response({ items: page.map(pt), nextCursor: next });
    }
  }
  if (parts[0] === "tasks" && parts[1]) {
    const t = task(c, parts[1]), p = s.projects.find(x => x.id === t.projectId)!;
    const u = user(c), sub = parts[2];
    if (!sub && method === "GET") return response(pt(t), 200, etag(t));
    if (!sub && method === "PATCH") {
      if (!editor(p, u.id)) fail(403, "forbidden", "Read only");
      match(c, t); change(c, t, await bodyOf(req)); return response(pt(t), 200, etag(t));
    }
    if (!sub && method === "DELETE") {
      if (!owner(p, u.id) && t.createdBy !== u.id) fail(403, "forbidden", "Read only");
      s.tasks.splice(s.tasks.indexOf(t), 1);
      s.tasks.forEach(x => x.deps.delete(t.id)); return response(null, 204);
    }
    if (sub === "transitions" && method === "POST") {
      if (!editor(p, u.id)) fail(403, "forbidden", "Read only");
      match(c, t);
      const b = await bodyOf(req); validate(["open", "in_progress", "done"].includes(b.to), "to");
      const valid = (t.status === "open" && b.to === "in_progress") ||
        (t.status === "in_progress" && ["open", "done"].includes(b.to)) ||
        (t.status === "done" && b.to === "open");
      if (!valid) fail(409, "invalid_transition", "Invalid transition");
      if (b.to === "done" && [...t.deps].some(n => s.tasks.some(x => x.id === n && x.status !== "done")))
        fail(409, "blocked", "Dependencies are incomplete");
      const old = t.status; t.status = b.to; t.version++; t.updatedAt = stamp(c);
      t.history.push({ at: t.updatedAt, by: u.id, field: "status", from: old, to: t.status });
      return response(pt(t), 200, etag(t));
    }
    if (sub === "history" && method === "GET") return response(t.history);
    if (sub === "comments") {
      if (method === "GET") return response(t.comments);
      if (method === "POST") {
        const b = await bodyOf(req);
        validate(typeof b.body === "string" && !!b.body.trim() && b.body.trim().length <= 2000, "body");
        const item = { id: ++s.cid, authorId: u.id, body: b.body.trim(), createdAt: stamp(c) };
        t.comments.push(item); return response(item, 201);
      }
    }
    if (sub === "dependencies") return dependencies(c, t, parts, method, p);
  }
  const methods = path === "/users" ? ["POST"] : path === "/me" ? ["GET"] :
    path === "/sessions" ? ["POST", "DELETE"] : path === "/projects" ? ["GET", "POST"] : [];
  if (methods.length && !methods.includes(method))
    return response({ error: { code: "method_not_allowed", message: "Method not allowed" } }, 405,
      { Allow: methods.join(", ") });
  fail(404, "not_found", "Not found");
}

