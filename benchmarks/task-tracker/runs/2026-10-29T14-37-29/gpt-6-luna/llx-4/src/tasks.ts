import { publicTask, taskPayload, valid } from "./taskModel";
import { listing } from "./taskListing";

type Context = {
  path: string[];
  method: string;
  body: any;
  request: Request;
  url: URL;
  tasks: any[];
  projects: any[];
  users: any[];
  userId?: number;
  token: string | null;
  idempotency: Map<string, { at: number; body: string; result: any }>;
  now: number;
  sequence: () => number;
};

const json = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  new Response(body === null ? null : JSON.stringify(body), {
    status, headers: { "Content-Type": "application/json", ...headers },
  });
const fail = (status: number, code: string, field?: string) =>
  json({ error: { code, message: code, ...(field ? { field } : {}) } }, status);

function visible(task: any, project: any, uid?: number): boolean {
  return !!uid && (project.ownerId === uid || project.members.some((m: any) => m.userId === uid));
}

export function handleTasks(c: Context): Response | null {
  const { path, method, body, request, url, tasks, projects, users, userId, token, now } = c;
  const isProject = path[0] === "projects" && path[2] === "tasks";
  const isTask = path[0] === "tasks";
  if (!isProject && !isTask) return null;
  if (!userId) return fail(401, "unauthorized");
  const project = isProject ? projects.find((p) => p.id === Number(path[1])) : null;
  if (isProject && (!project || !visible({} as any, project, userId))) return fail(404, "not_found");
  const role = project?.ownerId === userId ? "owner" :
    project?.members.find((m: any) => m.userId === userId)?.role;
  const records = isTask
    ? tasks.filter((t) => t.key === path[1] || t.key.startsWith(`${path[1]}-`))
    : tasks.filter((t) => t.projectId === project.id);
  if (isProject && path.length === 3 && method === "POST") {
    if (role === "viewer") return fail(403, "forbidden");
    const field = valid(body);
    if (field) return fail(400, "validation", field);
    if (body.assigneeId != null && !project.members.some((m: any) => m.userId === body.assigneeId) &&
      body.assigneeId !== project.ownerId) return fail(400, "validation", "assigneeId");
    const ik = request.headers.get("Idempotency-Key");
    const ikey = ik ? `${userId}:${ik}` : "";
    const digest = JSON.stringify(body);
    if (ik) {
      const prior = c.idempotency.get(ikey);
      if (prior && now - prior.at < 86_400_000) {
        if (prior.body !== digest) return fail(422, "idempotency_mismatch");
        return json(prior.result, 201, { ETag: `"${prior.result.version}"`, "Idempotent-Replayed": "true" });
      }
    }
    const made = taskPayload({ ...body, creatorId: userId }, project, tasks, now);
    const result = publicTask(made);
    if (ik) c.idempotency.set(ikey, { at: now, body: digest, result });
    return json(result, 201, { ETag: '"1"' });
  }
  if (isProject && path[3] === "bulk" && method === "POST") {
    if (role === "viewer") return fail(403, "forbidden");
    const batch = body.tasks;
    if (!Array.isArray(batch) || batch.length < 1 || batch.length > 50)
      return fail(400, "validation", "tasks");
    for (let i = 0; i < batch.length; i++) {
      const field = valid(batch[i]);
      if (field) return fail(400, "validation", `tasks[${i}].${field}`);
    }
    return json({ items: batch.map((x: any) =>
      publicTask(taskPayload({ ...x, creatorId: userId }, project, tasks, now))) }, 201);
  }
  if (isProject && method === "GET") {
    return listing(records, url, userId!);
  }
  const task = records[0];
  if (!task) return fail(404, "not_found");
  const p = projects.find((x) => x.id === task.projectId);
  if (!visible(task, p, userId)) return fail(404, "not_found");
  const taskRole = p.ownerId === userId ? "owner" :
    p.members.find((m: any) => m.userId === userId)?.role;
  if (path.length === 2 && method === "GET")
    return json(publicTask(task), 200, { ETag: `"${task.version}"` });
  if (path[2] === "history" && method === "GET") return json(task.history);
  if (path[2] === "dependencies" && method === "GET") {
    const blocks = tasks.filter((t) => t.dependencies.includes(task.key)).map((t) => t.key)
      .sort((a, b) => Number(a.split("-").pop()) - Number(b.split("-").pop()));
    return json({ key: task.key, dependsOn: task.dependencies.slice().sort((a: string, b: string) =>
      Number(a.split("-").pop()) - Number(b.split("-").pop())), blocks });
  }
  if (path[2] === "dependencies" && method === "POST") {
    if (taskRole === "viewer") return fail(403, "forbidden");
    const target = tasks.find((t) => t.key === body.dependsOn);
    if (!target || target.projectId !== task.projectId || target.key === task.key)
      return fail(400, "validation", "dependsOn");
    if (task.dependencies.includes(target.key)) return fail(409, "conflict");
    const reaches = (key: string, seen = new Set<string>()): boolean => {
      if (key === task.key) return true;
      if (seen.has(key)) return false;
      seen.add(key);
      const item = tasks.find((t) => t.key === key);
      return !!item?.dependencies.some((dep: string) => reaches(dep, seen));
    };
    if (reaches(target.key)) return fail(409, "dependency_cycle");
    task.dependencies.push(target.key);
    return json({ key: task.key, dependsOn: task.dependencies, blocks: [] }, 201);
  }
  if (path[2] === "dependencies" && path[3] && method === "DELETE") {
    const at = task.dependencies.indexOf(path[3]);
    if (at < 0) return fail(404, "not_found");
    task.dependencies.splice(at, 1);
    return json(null, 204);
  }
  if (path[2] === "transitions" && method === "POST") {
    if (taskRole === "viewer") return fail(403, "forbidden");
    const match = request.headers.get("If-Match");
    if (match && match !== `"${task.version}"`) return fail(412, "precondition_failed");
    if (!["open", "in_progress", "done"].includes(body.to))
      return fail(400, "validation", "to");
    const allowed = task.status === "open" ? body.to === "in_progress" :
      task.status === "in_progress" ? ["open", "done"].includes(body.to) : body.to === "open";
    if (!allowed) return fail(409, "invalid_transition");
    if (body.to === "done" && task.dependencies.some((key: string) =>
      tasks.some((t) => t.key === key && t.status !== "done"))) return fail(409, "blocked");
    const from = task.status;
    task.status = body.to;
    task.version++;
    task.history.push({ at: new Date(now).toISOString(), by: userId, field: "status", from, to: body.to });
    task.updatedAt = new Date(now).toISOString();
    return json(publicTask(task), 200, { ETag: `"${task.version}"` });
  }
  if (path[2] === "comments" && method === "GET") return json(task.comments);
  if (path[2] === "comments" && method === "POST") {
    const text = typeof body.body === "string" ? body.body.trim() : "";
    if (!text || text.length > 2000) return fail(400, "validation", "body");
    const comment = { id: task.comments.length + 1, authorId: userId, body: text,
      createdAt: new Date(now).toISOString() };
    task.comments.push(comment);
    return json(comment, 201);
  }
  if (method === "DELETE" && path.length === 2) {
    if (taskRole === "viewer" || (taskRole !== "owner" && task.creatorId !== userId))
      return fail(403, "forbidden");
    tasks.splice(tasks.indexOf(task), 1);
    tasks.forEach((item) => {
      item.dependencies = item.dependencies.filter((key: string) => key !== task.key);
    });
    return json(null, 204);
  }
  if (method === "PATCH" && path.length === 2) {
    if (taskRole === "viewer") return fail(403, "forbidden");
    const match = request.headers.get("If-Match");
    if (match && match !== `"${task.version}"`) return fail(412, "precondition_failed");
    for (const key of Object.keys(body)) {
      if (!["title", "description", "priority", "labels", "assigneeId"].includes(key))
        return fail(400, "validation", key);
    }
    const field = valid({ ...task, ...body });
    if (field) return fail(400, "validation", field);
    let changed = false;
    for (const key of Object.keys(body).sort()) {
      const value = body[key];
      const next = key === "labels" ? [...new Set((value as string[]).map((x) =>
        x.trim().toLowerCase()))].sort() : value;
      if (JSON.stringify(task[key]) !== JSON.stringify(next)) {
        task.history.push({ at: new Date(now).toISOString(), by: userId, field: key,
          from: task[key] ?? null, to: next });
        task[key] = next;
        changed = true;
      }
    }
    if (changed) task.version++;
    if (changed) task.updatedAt = new Date(now).toISOString();
    return json(publicTask(task), 200, { ETag: `"${task.version}"` });
  }
  return null;
}

