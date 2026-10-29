import type { State } from "./state";
import {
  auth, error, invalid, isError, json, normalizedLabels, projectAccess,
  role, taskAccess, taskFields, taskFor,
} from "./http";
import { change, patch, version } from "./task-changes";

type C = {
  request: Request; state: State; method: string; path: string; input: any; url: URL;
};
export async function tasks(c: C): Promise<Response | undefined> {
  const { state, method, path, input, request, url } = c;
  const create = path.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if (create && method === "POST") {
    const user = auth(request, state);
    if (isError(user)) return user;
    const projectId = Number(create[1]);
    const project = projectAccess(state, projectId, user.id);
    if (!project) return error("not_found", "Project not found", 404);
    if (role(state, projectId, user.id) === "viewer") return error("forbidden", "Forbidden", 403);
    const bulk = Boolean(create[2]);
    const key = request.headers.get("Idempotency-Key");
    const idemKey = key ? `${user.id}:${projectId}:${key}` : "";
    const prior = idemKey ? state.idem.get(idemKey) : undefined;
    if (prior && state.now() - prior.at < 86400000) {
      if (prior.body !== JSON.stringify(input))
        return error("idempotency_mismatch", "Idempotency key body differs", 422);
      return json(prior.task, 201, { "Idempotent-Replayed": "true", ETag: '"1"' });
    }
    if (prior) state.idem.delete(idemKey);
    const inputs = bulk ? input?.tasks : [input];
    if (bulk && (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > 50))
      return invalid("tasks");
    if (!Array.isArray(inputs)) return invalid("tasks");
    for (let i = 0; i < inputs.length; i++) {
      const field = taskFields(inputs[i] ?? {}, user.id, state, projectId);
      if (field) return invalid(bulk ? `tasks[${i}].${field}` : field);
    }
    const results = inputs.map((item: any) => {
      const number = state.tasks.filter((t) => t.projectId === projectId).length + 1;
      const now = state.iso();
      const task = {
        id: ++state.ids.task, key: `${project.key}-${number}`, number, projectId,
        title: item.title.trim(), description: item.description ?? "", status: "open",
        priority: item.priority ?? "normal", labels: normalizedLabels(item.labels),
        assigneeId: item.assigneeId ?? null, createdBy: user.id, createdAt: now,
        updatedAt: now, version: 1,
      };
      state.tasks.push(task);
      state.history.set(task.id, []);
      state.comments.set(task.id, []);
      state.dependencies.set(task.id, new Set());
      return task;
    });
    if (idemKey) state.idem.set(idemKey, {
      body: JSON.stringify(input), task: results[0]!, at: state.now(),
    });
    return bulk ? json({ items: results }, 201) : json(results[0]!, 201, { ETag: '"1"' });
  }
  const list = path.match(/^\/projects\/(\d+)\/tasks$/);
  if (list && method === "GET") {
    const user = auth(request, state);
    if (isError(user)) return user;
    const id = Number(list[1]);
    if (!projectAccess(state, id, user.id)) return error("not_found", "Project not found", 404);
    const all = state.tasks.filter((t) => t.projectId === id && !t.deleted);
    const status = url.searchParams.get("status");
    if (status && !["open", "in_progress", "done"].includes(status)) return invalid("status");
    const sort = url.searchParams.get("sort") ?? "created";
    if (!["created", "-created", "priority"].includes(sort)) return invalid("sort");
    const limitText = url.searchParams.get("limit");
    const limit = limitText === null ? 20 : Number(limitText);
    if (!Number.isInteger(limit) || limit < 1 || limit > 100) return invalid("limit");
    let items = all.filter((t) => !status || t.status === status);
    const assignee = url.searchParams.get("assignee");
    if (assignee) {
      const target = assignee === "me" ? user.id
        : assignee === "none" ? null : Number(assignee);
      if (target === null && assignee === "none") items = items.filter((t) => !t.assigneeId);
      else if (typeof target === "number" && Number.isFinite(target))
        items = items.filter((t) => t.assigneeId === target);
      else return invalid("assignee");
    }
    const label = url.searchParams.get("label")?.toLowerCase();
    if (label) items = items.filter((t) => t.labels.includes(label));
    const q = url.searchParams.get("q")?.toLowerCase();
    if (q) items = items.filter((t) => `${t.title} ${t.description}`.toLowerCase().includes(q));
    items.sort((a, b) => sort === "priority"
      ? ["high", "normal", "low"].indexOf(a.priority) -
        ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number
      : (sort === "-created" ? b.number - a.number : a.number - b.number));
    const cursor = url.searchParams.get("cursor");
    if (cursor) {
      try {
        const marker = JSON.parse(atob(cursor));
        if (marker.id !== id) throw Error();
        items = items.slice(marker.offset);
      }
      catch { return invalid("cursor"); }
    }
    const page = items.slice(0, limit);
    const offset = cursor ? JSON.parse(atob(cursor)).offset : 0;
    const nextCursor = items.length > limit ? btoa(JSON.stringify({
      id, offset: offset + limit,
    })) : null;
    return json({ items: page, nextCursor });
  }
  const match = path.match(/^\/tasks\/([^/]+)(?:\/(.*))?$/);
  if (!match) return undefined;
  const user = auth(request, state);
  if (isError(user)) return user;
  const task = taskFor(state, match[1]!);
  if (!task || !taskAccess(state, task, user.id)) return error("not_found", "Task not found", 404);
  const suffix = match[2] ?? "";
  if (!suffix && method === "GET") return json(task, 200, { ETag: `"${task.version}"` });
  if (!suffix && method === "DELETE") {
    if (role(state, task.projectId, user.id) !== "owner" && task.createdBy !== user.id)
      return error("forbidden", "Forbidden", 403);
    task.deleted = true;
    state.dependencies.forEach((set) => set.delete(task.id));
    return new Response(null, { status: 204 });
  }
  if (!suffix && method === "PATCH") return patch(c, task, user.id);
  if (suffix === "transitions" && method === "POST") {
    if (role(state, task.projectId, user.id) === "viewer") return error("forbidden", "Forbidden", 403);
    const check = version(request, task.version);
    if (check) return check;
    if (!["open", "in_progress", "done"].includes(input?.to)) return invalid("to");
    const valid = task.status === "open" && input.to === "in_progress" ||
      task.status === "in_progress" && ["open", "done"].includes(input.to) ||
      task.status === "done" && input.to === "open";
    if (!valid) return error("invalid_transition", "Invalid transition", 409);
    const blocked = [...(state.dependencies.get(task.id) ?? [])]
      .some((id) => state.tasks.find((t) => t.id === id && !t.deleted)?.status !== "done");
    if (input.to === "done" && blocked) return error("blocked", "Task has unfinished dependencies", 409);
    change(state, task, user.id, "status", input.to);
    return json(task, 200, { ETag: `"${task.version}"` });
  }
  if (suffix === "history" && method === "GET") return json(state.history.get(task.id) ?? []);
  const deps = suffix.match(/^dependencies(?:\/([^/]+))?$/);
  if (deps) {
    const related = [...(state.dependencies.get(task.id) ?? [])]
      .map((id) => state.tasks.find((t) => t.id === id && !t.deleted))
      .filter((item) => item !== undefined)
      .sort((a, b) => a.number - b.number);
    if (method === "GET") {
      const blocks = state.tasks.filter((t) =>
        !t.deleted && state.dependencies.get(t.id)?.has(task.id),
      ).sort((a, b) => a.number - b.number);
      return json({
        key: task.key, dependsOn: related.map((t) => t.key),
        blocks: blocks.map((t) => t.key),
      });
    }
    if (method === "POST") {
      if (role(state, task.projectId, user.id) === "viewer")
        return error("forbidden", "Forbidden", 403);
      const other = taskFor(state, input?.dependsOn);
      if (!other || other.projectId !== task.projectId || other.id === task.id)
        return invalid("dependsOn");
      const set = state.dependencies.get(task.id)!;
      if (set.has(other.id)) return error("conflict", "Dependency exists", 409);
      const reaches = (from: number, target: number): boolean =>
        from === target || [...(state.dependencies.get(from) ?? [])]
          .some((next) => reaches(next, target));
      if (reaches(other.id, task.id)) return error("dependency_cycle", "Dependency cycle", 409);
      set.add(other.id);
      return json({
        key: task.key,
        dependsOn: [...set].map((n) => state.tasks.find((t) => t.id === n)?.key)
          .filter(Boolean).sort(),
        blocks: [],
      }, 201);
    }
    if (method === "DELETE" && deps[1]) {
      const other = taskFor(state, deps[1]);
      if (!other || !state.dependencies.get(task.id)!.delete(other.id))
        return error("not_found", "Dependency not found", 404);
      return new Response(null, { status: 204 });
    }
  }
  if (suffix === "comments" && method === "GET") return json(state.comments.get(task.id) ?? []);
  if (suffix === "comments" && method === "POST") {
    const text = typeof input?.body === "string" ? input.body.trim() : "";
    if (!text || text.length > 2000) return invalid("body");
    const comment = { id: (state.comments.get(task.id)?.length ?? 0) + 1,
      authorId: user.id, body: text, createdAt: state.iso() };
    state.comments.get(task.id)!.push(comment);
    return json(comment, 201);
  }
  return undefined;
}

