import type { State } from "./state";
import { auth, error, invalid, isError, json, member, projectAccess } from "./http";

type C = { request: Request; state: State; method: string; path: string; input: any };
export async function projects(c: C): Promise<Response | undefined> {
  const { state, method, path, input, request } = c;
  if (path === "/projects" && method === "POST") {
    const user = auth(request, state);
    if (isError(user)) return user;
    if (typeof input?.name !== "string" || !input.name.trim() || input.name.trim().length > 100)
      return invalid("name");
    if (typeof input.key !== "string" || !/^[A-Z]{2,6}$/.test(input.key)) return invalid("key");
    if (state.projects.some((p) => p.key === input.key))
      return error("conflict", "Project key already exists", 409, "key");
    const project = {
      id: ++state.ids.project, name: input.name.trim(), key: input.key,
      ownerId: user.id, createdAt: state.iso(),
    };
    state.projects.push(project);
    state.members.set(project.id, new Map([[user.id, "owner"]]));
    return json(project, 201);
  }
  if (path === "/projects" && method === "GET") {
    const user = auth(request, state);
    if (isError(user)) return user;
    return json(state.projects.filter((p) => member(state, p.id, user.id)));
  }
  const match = path.match(/^\/projects\/(\d+)(?:\/(members)(?:\/(\d+))?|\/(stats))?$/);
  if (!match) return undefined;
  const user = auth(request, state);
  if (isError(user)) return user;
  const id = Number(match[1]);
  const p = projectAccess(state, id, user.id);
  if (!p) return error("not_found", "Project not found", 404);
  if (match[4] === "stats" && method === "GET") {
    const list = state.tasks.filter((t) => t.projectId === id && !t.deleted);
    const bucket = (name: string) => Object.fromEntries(name === "status"
      ? ["open", "in_progress", "done"].map((v) => [v, list.filter((t) => t.status === v).length])
      : ["high", "normal", "low"].map((v) => [v, list.filter((t) => t.priority === v).length]));
    const assignees = new Map<number, number>();
    list.forEach((t) => { if (t.assigneeId) assignees.set(t.assigneeId,
      (assignees.get(t.assigneeId) ?? 0) + 1); });
    return json({
      total: list.length, byStatus: bucket("status"), byPriority: bucket("priority"),
      byAssignee: [...assignees].map(([userId, count]) => ({ userId, count }))
        .sort((a, b) => b.count - a.count || a.userId - b.userId),
    });
  }
  if (!match[2] && !match[4] && method === "GET") return json(p);
  if (match[2] === "members" && !match[3] && method === "GET") {
    const ids = [...(state.members.get(id) ?? new Map())];
    return json(ids.sort((a, b) => a[1] === "owner" ? -1 : b[1] === "owner" ? 1 : a[0] - b[0])
      .map(([userId, role]) => ({ userId, role })));
  }
  if (match[2] === "members" && method === "POST") {
    if (state.members.get(id)?.get(user.id) !== "owner") return error("forbidden", "Forbidden", 403);
    const target = input?.userId;
    if (!Number.isInteger(target) || !state.users.some((x) => x.id === target)) return invalid("userId");
    if (!["member", "viewer"].includes(input?.role)) return invalid("role");
    if (member(state, id, target)) return error("conflict", "Already a member", 409);
    state.members.get(id)!.set(target, input.role);
    return json({ userId: target, role: input.role }, 201);
  }
  if (match[2] === "members" && match[3] && method === "DELETE") {
    const target = Number(match[3]);
    if (state.members.get(id)?.get(user.id) !== "owner") return error("forbidden", "Forbidden", 403);
    if (!state.members.get(id)?.has(target) || target === p.ownerId)
      return error("not_found", "Member not found", 404);
    state.members.get(id)!.delete(target);
    state.tasks.filter((t) => t.projectId === id && t.assigneeId === target)
      .forEach((t) => { t.assigneeId = null; });
    return new Response(null, { status: 204 });
  }
  return undefined;
}
