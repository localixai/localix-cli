import type { State, Task } from "./model";
import { iso, taskView } from "./model";
import { bump, record } from "./task-history";
import { dependencies } from "./task-dependencies";
import {
  ApiError,
  body,
  checkVersion,
  etag,
  respond,
  taskAccess,
  writer,
} from "./http";
export async function taskDetail(
  req: Request,
  path: string,
  s: State,
  uid: number,
): Promise<Response | null> {
  const match =
    /^\/tasks\/([^/]+)(?:\/(history|transitions|comments|dependencies)(?:\/([^/]+))?)?$/.exec(
      path,
    );
  if (!match) return null;
  const key = decodeURIComponent(match[1]!),
    sub = match[2],
    tail = match[3];
  const t = taskAccess(s, key, uid),
    method = req.method;
  if (!sub && method === "GET") return respond(taskView(t), 200, etag(t));
  if (!sub && method === "DELETE") {
    const role = (await import("./http")).member(
      s,
      (await import("./model")).projectOf(s, t.projectId)!,
      uid,
    );
    if (role !== "owner" && (role !== "member" || t.createdBy !== uid))
      throw new ApiError(403, "forbidden", "cannot delete this task");
    s.tasks = s.tasks.filter((x) => x.id !== t.id);
    for (const x of s.tasks) x.dependsOn.delete(t.id);
    return respond(null, 204);
  }
  if (!sub && method === "PATCH") {
    writer(s, t, uid);
    checkVersion(req, t);
    const b = await body(req);
    const allowed = [
      "title",
      "description",
      "priority",
      "labels",
      "assigneeId",
    ];
    for (const k of Object.keys(b))
      if (!allowed.includes(k))
        throw new ApiError(400, "validation", `${k} is invalid`, k);
    const normalized: Record<string, unknown> = {};
    for (const k of allowed)
      if (k in b) {
        const v = b[k];
        if (
          k === "title" &&
          (typeof v !== "string" || !v.trim() || v.trim().length > 200)
        )
          throw new ApiError(400, "validation", "title is invalid", "title");
        if (k === "description" && (typeof v !== "string" || v.length > 10000))
          throw new ApiError(
            400,
            "validation",
            "description is invalid",
            "description",
          );
        if (k === "priority" && !["high", "normal", "low"].includes(v))
          throw new ApiError(
            400,
            "validation",
            "priority is invalid",
            "priority",
          );
        if (
          k === "labels" &&
          (!Array.isArray(v) || v.some((x: unknown) => typeof x !== "string"))
        )
          throw new ApiError(400, "validation", "labels are invalid", "labels");
        if (k === "assigneeId" && v !== null && !Number.isInteger(v))
          throw new ApiError(
            400,
            "validation",
            "assigneeId is invalid",
            "assigneeId",
          );
        normalized[k] =
          k === "title"
            ? (v as string).trim()
            : k === "labels"
              ? [
                  ...new Set(
                    (v as string[])
                      .map((x) => x.trim().toLowerCase())
                      .filter(Boolean),
                  ),
                ].sort()
              : v;
      }
    if ("assigneeId" in normalized && normalized.assigneeId !== null) {
      const p = (await import("./model")).projectOf(s, t.projectId)!;
      if (
        !(await import("./http")).member(s, p, normalized.assigneeId as number)
      )
        throw new ApiError(
          400,
          "validation",
          "assigneeId is invalid",
          "assigneeId",
        );
    }
    const fields = Object.keys(normalized).sort();
    let changed = false;
    for (const k of fields) {
      const old = (t as any)[k],
        value = normalized[k];
      if (JSON.stringify(old) !== JSON.stringify(value)) {
        record(s, t, uid, k === "assigneeId" ? "assigneeId" : k, old, value);
        (t as any)[k] = value;
        changed = true;
      }
    }
    if (changed) bump(s, t);
    return respond(taskView(t), 200, etag(t));
  }
  if (sub === "history" && method === "GET") return respond(t.history);
  if (sub === "transitions" && method === "POST") {
    writer(s, t, uid);
    checkVersion(req, t);
    const b = await body(req),
      to = b.to;
    if (!["open", "in_progress", "done"].includes(to))
      throw new ApiError(400, "validation", "to is invalid", "to");
    const valid =
      (t.status === "open" && to === "in_progress") ||
      (t.status === "in_progress" && (to === "open" || to === "done")) ||
      (t.status === "done" && to === "open");
    if (!valid)
      throw new ApiError(
        409,
        "invalid_transition",
        "transition is not allowed",
      );
    if (
      to === "done" &&
      [...t.dependsOn].some((id) =>
        s.tasks.some((x) => x.id === id && x.status !== "done"),
      )
    )
      throw new ApiError(409, "blocked", "task has unfinished dependencies");
    record(s, t, uid, "status", t.status, to);
    t.status = to;
    bump(s, t);
    return respond(taskView(t), 200, etag(t));
  }
  if (sub === "comments") {
    if (method === "GET") return respond(t.comments);
    if (method === "POST") {
      const b = await body(req),
        text = typeof b.body === "string" ? b.body.trim() : "";
      if (!text || text.length > 2000)
        throw new ApiError(400, "validation", "body is invalid", "body");
      const c = {
        id: t.comments.length + 1,
        authorId: uid,
        body: text,
        createdAt: iso(s),
      };
      t.comments.push(c);
      return respond(c, 201);
    }
  }
  if (sub === "dependencies") return dependencies(req, t, tail, s, uid);
  return null;
}
