import type { Ctx, Result, Task } from "./types";
import {
  cleanLabels,
  etag,
  fail,
  findTask,
  iso,
  member,
  next,
  ok,
  projectAccess,
  taskAccess,
} from "./util";
import { comments, patchTask, transition } from "./taskchanges";
import { listRoute } from "./listing";
import { dependencyRoute } from "./dependencies";
function validate(c: Ctx, b: any, p: number, base = ""): Result | null {
  const prefix = base ? `${base}.` : "";
  if (
    typeof b?.title !== "string" ||
    !b.title.trim() ||
    b.title.trim().length > 200
  )
    return fail(400, "validation", "invalid title", prefix + "title");
  if (b.description !== undefined && typeof b.description !== "string")
    return fail(
      400,
      "validation",
      "invalid description",
      prefix + "description",
    );
  if (
    b.priority !== undefined &&
    !["low", "normal", "high"].includes(b.priority)
  )
    return fail(400, "validation", "invalid priority", prefix + "priority");
  if (b.labels !== undefined && cleanLabels(b.labels) === null)
    return fail(400, "validation", "invalid labels", prefix + "labels");
  if (
    b.assigneeId !== undefined &&
    b.assigneeId !== null &&
    !c.state.members.get(p)?.has(b.assigneeId)
  )
    return fail(400, "validation", "invalid assignee", prefix + "assigneeId");
  for (const k of Object.keys(b))
    if (
      !["title", "description", "priority", "labels", "assigneeId"].includes(k)
    )
      return fail(400, "validation", "unknown field", prefix + k);
  return null;
}
export function createTask(c: Ctx, p: number, b: any): Result {
  const e = validate(c, b, p);
  if (e) return e;
  const project = c.state.projects.find((x) => x.id === p)!;
  const number = (c.state.ids[`task-${p}`] || 0) + 1;
  c.state.ids[`task-${p}`] = number;
  const t: Task = {
    id: next(c, "task"),
    key: `${project.key}-${number}`,
    number,
    projectId: p,
    title: b.title.trim(),
    description: b.description || "",
    status: "open",
    priority: b.priority || "normal",
    labels: cleanLabels(b.labels || [])!,
    assigneeId: b.assigneeId ?? null,
    createdBy: c.user!.id,
    createdAt: iso(c),
    updatedAt: iso(c),
    version: 1,
  };
  c.state.tasks.push(t);
  c.state.histories.set(t.id, []);
  c.state.deps.set(t.id, new Set());
  c.state.comments.set(t.id, []);
  return ok(t, 201, etag(t));
}
export function taskRoute(c: Ctx, path: string, m: string): Result | null {
  const s = c.state,
    b = c.body;
  let x = path.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if (x) {
    const p = Number(x[1]);
    if (!projectAccess(c, p)) return fail(404, "not_found");
    if (x[2]) return null;
    if (m === "POST") {
      if (member(c, p) === "viewer") return fail(403, "forbidden");
      const ik = c.req.headers.get("Idempotency-Key");
      if (ik) {
        const k = `${c.user!.id}:${ik}`,
          old = c.state.idem.get(k),
          finger = JSON.stringify([p, b]);
        if (old && c.state.now() - old.at < 86400000) {
          if (old.finger !== finger) return fail(422, "idempotency_mismatch");
          return {
            ...old.result,
            headers: {
              ...(old.result.headers || {}),
              "Idempotent-Replayed": "true",
            },
          };
        }
        const result = createTask(c, p, b);
        if (result.status === 201)
          c.state.idem.set(k, { at: c.state.now(), finger, result });
        return result;
      }
      return createTask(c, p, b);
    }
    if (m === "GET") return listRoute(c, p);
  }
  x = path.match(
    /^\/tasks\/([^/]+)(?:\/(history|comments|transitions|dependencies)(?:\/([^/]+))?)?$/,
  );
  if (!x) return null;
  const key = decodeURIComponent(x[1]!),
    sub = x[2],
    tail = x[3],
    t = taskAccess(c, key);
  if (!t) return fail(404, "not_found");
  if (!sub && m === "GET") return ok(t, 200, etag(t));
  if (sub === "history" && m === "GET") return ok(s.histories.get(t.id) || []);
  if (sub === "transitions" && m === "POST") {
    const match = c.req.headers.get("If-Match");
    if (match && match !== `"${t.version}"`)
      return fail(412, "precondition_failed");
    return transition(c, t, b);
  }
  if ((!sub && m === "PATCH") || (sub === "transitions" && m === "POST")) {
    const match = c.req.headers.get("If-Match");
    if (match && match !== `"${t.version}"`)
      return fail(412, "precondition_failed");
  }
  if (!sub && m === "PATCH") return patchTask(c, t, b);
  if (!sub && m === "DELETE") {
    if (member(c, t.projectId) === "viewer") return fail(403, "forbidden");
    if (t.createdBy !== c.user!.id && member(c, t.projectId) !== "owner")
      return fail(403, "forbidden");
    s.tasks = s.tasks.filter((a) => a !== t);
    for (const d of s.deps.values()) d.delete(t.id);
    s.deps.delete(t.id);
    return ok(undefined, 204);
  }
  if (sub === "comments") return comments(c, t, m, b);
  if (sub === "dependencies") return dependencyRoute(c, t, m, tail);
  return null;
}
