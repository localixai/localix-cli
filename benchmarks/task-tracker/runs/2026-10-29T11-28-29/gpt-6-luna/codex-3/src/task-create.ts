import type { State, Task } from "./model";
import { auth, bad, bodyOf, iso, json, member, projectFor, publicTask, visibleRole } from "./http";
export function validateTask(state: State, projectId: number, b: any, prefix = "") {
  const field = (name: string) => prefix ? `${prefix}.${name}` : name;
  if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200) bad(field("title"));
  if (b.description !== undefined && typeof b.description !== "string") bad(field("description"));
  if (b.priority !== undefined && !["low", "normal", "high"].includes(b.priority)) bad(field("priority"));
  let labels: string[] = [];
  if (b.labels !== undefined) {
    if (!Array.isArray(b.labels)) bad(field("labels"));
    labels = b.labels.map((v: unknown) => typeof v === "string" ? v.trim().toLowerCase() : "");
    if (labels.some((v) => !/^[a-z0-9_-]{1,30}$/.test(v))) bad(field("labels"));
  }
  if (b.assigneeId !== undefined && b.assigneeId !== null &&
    !visibleRole(state, projectId, b.assigneeId)) bad(field("assigneeId"));
  return { title: b.title.trim(), description: b.description ?? "", priority: b.priority ?? "normal",
    labels: [...new Set(labels)].sort(), assigneeId: b.assigneeId ?? null };
}
export async function createTasks(state: State, request: Request, projectId: number, bulk: boolean) {
  const uid = auth(state, request);
  projectFor(state, projectId, uid);
  member(state, projectId, uid);
  const b = await bodyOf(request);
  let rows: any[];
  if (bulk) {
    if (!Array.isArray(b.tasks) || b.tasks.length < 1 || b.tasks.length > 50) bad("tasks");
    rows = b.tasks;
  } else rows = [b];
  const idempotencyKey = !bulk ? request.headers.get("Idempotency-Key") : null;
  const replayKey = idempotencyKey ? `${uid}:${projectId}:${idempotencyKey}` : "";
  const cleanBody = JSON.stringify(bulk ? b : b);
  if (replayKey) {
    const prior = state.idempotency.get(replayKey);
    if (prior && state.now() - prior.at < 86400000) {
      if (prior.body !== cleanBody) {
        const { fail } = await import("./http");
        fail(422, "idempotency_mismatch", "Idempotency key reused with another body");
      }
      return json(prior.task, 201, { "Idempotent-Replayed": "true", ETag: `"1"` });
    }
  }
  const specs = rows.map((row, i) => validateTask(state, projectId, row, bulk ? `tasks[${i}]` : ""));
  const project = state.projects.find((p) => p.id === projectId)!;
  const made: Task[] = specs.map((spec) => {
    const number = state.nextNumbers.get(projectId) ?? 1;
    state.nextNumbers.set(projectId, number + 1);
    const now = iso(state);
    const task: Task = { id: state.nextTask++, key: `${project.key}-${number}`, number, projectId,
      ...spec, status: "open", createdBy: uid, createdAt: now, updatedAt: now, version: 1,
      dependsOn: [], comments: [], history: [] };
    state.tasks.push(task);
    return task;
  });
  const output = made.map(publicTask);
  const headers: Record<string, string> = made.length === 1 ? { ETag: '"1"' } : {};
  const response = json(bulk ? { items: output } : output[0], 201, headers);
  if (replayKey) state.idempotency.set(replayKey, { at: state.now(), body: cleanBody,
    task: output[0] });
  return response;
}
export function listTasks(state: State, request: Request, url: URL, projectId: number) {
  const uid = auth(state, request);
  projectFor(state, projectId, uid);
  const q = url.searchParams;
  const status = q.get("status");
  const sort = q.get("sort") ?? "created";
  const limitRaw = q.get("limit") ?? "20";
  const limit = Number(limitRaw);
  const assignee = q.get("assignee");
  const allowed = ["created", "-created", "priority"];
  if (status && !["open", "in_progress", "done"].includes(status)) bad("status");
  if (!allowed.includes(sort)) bad("sort");
  if (!/^\d+$/.test(limitRaw) || limit < 1 || limit > 100) bad("limit");
  if (q.has("cursor") && !/^\d+$/.test(q.get("cursor")!)) bad("cursor");
  if (assignee && assignee !== "me" && assignee !== "none" && !/^\d+$/.test(assignee)) bad("assignee");
  let tasks = state.tasks.filter((t) => t.projectId === projectId);
  if (status) tasks = tasks.filter((t) => t.status === status);
  if (assignee === "none") tasks = tasks.filter((t) => t.assigneeId === null);
  if (assignee === "me") tasks = tasks.filter((t) => t.assigneeId === uid);
  if (assignee && /^\d+$/.test(assignee)) tasks = tasks.filter((t) => t.assigneeId === Number(assignee));
  const label = q.get("label")?.toLowerCase();
  if (label) tasks = tasks.filter((t) => t.labels.includes(label));
  const query = q.get("q")?.toLowerCase();
  if (query) tasks = tasks.filter((t) => `${t.title} ${t.description}`.toLowerCase().includes(query));
  if (sort === "-created") tasks.reverse();
  if (sort === "priority") tasks.sort((a, b) => ["high", "normal", "low"].indexOf(a.priority) -
    ["high", "normal", "low"].indexOf(b.priority) || a.number - b.number);
  const offset = Number(q.get("cursor") ?? 0);
  const page = tasks.slice(offset, offset + limit);
  const nextCursor = offset + limit < tasks.length ? String(offset + limit) : null;
  return json({ items: page.map(publicTask), nextCursor });
}
