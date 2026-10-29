import type { State, Task } from "./model";
import { iso, projectOf, taskView } from "./model";
import { ApiError, body, member, projectAccess, respond } from "./http";
import { listTasks } from "./task-list";
export function normalize(
  s: State,
  p: { id: number },
  b: Record<string, any>,
  creator: number,
  number: number,
): Task {
  const title = typeof b.title === "string" ? b.title.trim() : "";
  if (!title || title.length > 200)
    throw new ApiError(400, "validation", "title is invalid", "title");
  const description = b.description === undefined ? "" : b.description;
  if (typeof description !== "string" || description.length > 10000)
    throw new ApiError(
      400,
      "validation",
      "description is invalid",
      "description",
    );
  const priority = b.priority ?? "normal";
  if (!["high", "normal", "low"].includes(priority))
    throw new ApiError(400, "validation", "priority is invalid", "priority");
  const labels = b.labels ?? [];
  if (!Array.isArray(labels) || labels.some((x) => typeof x !== "string"))
    throw new ApiError(400, "validation", "labels are invalid", "labels");
  if (labels.some((x: string) => x.trim().length > 30 || /\s/.test(x.trim()))) {
    throw new ApiError(400, "validation", "labels are invalid", "labels");
  }
  const cleanLabels = [
    ...new Set(
      labels.map((x: string) => x.trim().toLowerCase()).filter(Boolean),
    ),
  ].sort();
  let assigneeId = b.assigneeId ?? null;
  if (
    assigneeId !== null &&
    (!Number.isInteger(assigneeId) ||
      !member(s, projectOf(s, p.id)!, assigneeId))
  ) {
    throw new ApiError(
      400,
      "validation",
      "assigneeId is invalid",
      "assigneeId",
    );
  }
  const time = iso(s);
  return {
    id: s.tasks.length + 1,
    projectId: p.id,
    key: `${projectOf(s, p.id)!.key}-${number}`,
    number,
    title,
    description,
    status: "open",
    priority,
    labels: cleanLabels,
    assigneeId,
    createdBy: creator,
    createdAt: time,
    updatedAt: time,
    version: 1,
    history: [],
    comments: [],
    dependsOn: new Set(),
  };
}
export function createTask(
  s: State,
  p: { id: number },
  b: Record<string, any>,
  uid: number,
): Task {
  const number = (s.counters.get(p.id) || 0) + 1;
  s.counters.set(p.id, number);
  const t = normalize(s, p, b, uid, number);
  s.tasks.push(t);
  return t;
}
export async function taskCollection(
  req: Request,
  url: URL,
  s: State,
  id: number,
  bulk: boolean,
): Promise<Response> {
  const uid = Number((req as any).uid),
    p = projectAccess(s, id, uid);
  if (bulk) {
    if (!["owner", "member"].includes(member(s, p, uid) || ""))
      throw new ApiError(403, "forbidden", "write access required");
    const b = await body(req),
      list = b.tasks;
    if (!Array.isArray(list) || list.length < 1 || list.length > 50)
      throw new ApiError(400, "validation", "tasks is invalid", "tasks");
    // Validate all before writing, preserving the first failing field.
    const prepared: Task[] = [];
    for (let i = 0; i < list.length; i++) {
      try {
        prepared.push(
          normalize(
            s,
            p,
            list[i],
            uid,
            s.tasks.filter((t) => t.projectId === id).length + i + 1,
          ),
        );
      } catch (e) {
        if (e instanceof ApiError && e.field)
          e.field = `tasks[${i}].${e.field}`;
        throw e;
      }
    }
    s.tasks.push(...prepared);
    return respond({ items: prepared.map(taskView) }, 201);
  }
  if (req.method === "POST") {
    if (!["owner", "member"].includes(member(s, p, uid) || ""))
      throw new ApiError(403, "forbidden", "write access required");
    const b = await body(req),
      idem = req.headers.get("Idempotency-Key");
    const key = idem ? `${uid}:${idem}` : "";
    const previous = key ? s.idem.get(key) : undefined;
    if (previous && s.now() - previous.time < 86400000) {
      if (previous.body !== JSON.stringify(b))
        throw new ApiError(
          422,
          "idempotency_mismatch",
          "key used with another body",
        );
      return respond(previous.response, 201, {
        "Idempotent-Replayed": "true",
        ETag: '"1"',
      });
    }
    const t = createTask(s, p, b, uid),
      view = taskView(t);
    if (key)
      s.idem.set(key, {
        body: JSON.stringify(b),
        response: view,
        time: s.now(),
      });
    return respond(view, 201, { ETag: '"1"' });
  }
  return listTasks(url, s, id, uid);
}
