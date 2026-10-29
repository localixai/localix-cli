import { type Ctx, auth, body, changed, etag, fail, match, ok, taskByKey, valid } from "./http";
import type { Task } from "./model";
import { iso } from "./model";
export function makeTask(c: Ctx, pid: number, b: any, user: number): Task {
  valid(typeof b.title === "string" && b.title.trim().length > 0 && b.title.trim().length <= 200, "title");
  valid(b.description === undefined || typeof b.description === "string", "description");
  valid(b.priority === undefined || ["low", "normal", "high"].includes(b.priority), "priority");
  valid(b.labels === undefined || (Array.isArray(b.labels) && b.labels.every((x: any) =>
    typeof x === "string" && x.trim().length <= 30 && !/\s/.test(x.trim()))), "labels");
  const members = c.s.members.get(pid)!;
  valid(b.assigneeId === undefined || b.assigneeId === null ||
    (members.has(b.assigneeId) && c.s.users.some((u) => u.id === b.assigneeId)), "assigneeId");
  const p = c.s.projects.find((x) => x.id === pid)!;
  const number = (c.s.nextTask.get(pid) ?? 0) + 1;
  c.s.nextTask.set(pid, number);
  const at = iso(c.s);
  const t: Task = {
    id: ++c.s.ids.task, key: `${p.key}-${number}`, number, projectId: pid,
    title: b.title.trim(), description: b.description ?? "", status: "open",
    priority: b.priority ?? "normal",
    labels: [...new Set<string>((b.labels ?? []).map((x: string) =>
      x.trim().toLowerCase()))].sort(),
    assigneeId: b.assigneeId ?? null, createdBy: user, createdAt: at,
    updatedAt: at, version: 1, deps: [],
  };
  c.s.tasks.push(t); c.s.history.set(t.key, []); c.s.comments.set(t.key, []);
  return t;
}
export function view(t: Task) {
  const { deps, ...out } = t;
  return out;
}
export async function taskReadWrite(c: Ctx, path: string, method: string): Promise<Response | null> {
  const key = path.split("/")[2];
  if (!path.startsWith("/tasks/") || !key) return null;
  const t = taskByKey(c, key);
  if (path === `/tasks/${key}` && method === "GET") return ok(view(t), 200, etag(t));
  if (path === `/tasks/${key}` && method === "PATCH") {
    const user = auth(c, "member", t.projectId); match(c, t);
    const b = await body(c), allowed = ["title", "description", "priority", "labels", "assigneeId"];
    for (const k of Object.keys(b)) valid(allowed.includes(k), k);
    const patch = makePatch(c, t, b, user);
    return ok(view(patch), 200, etag(patch));
  }
  return null;
}
function makePatch(c: Ctx, t: Task, b: any, user: number): Task {
  const fields = ["title", "description", "priority", "labels", "assigneeId"].sort();
  const original: any = { ...t };
  if (b.title !== undefined) valid(typeof b.title === "string" && b.title.trim().length > 0 &&
    b.title.trim().length <= 200, "title");
  if (b.description !== undefined) valid(typeof b.description === "string", "description");
  if (b.priority !== undefined) valid(["low", "normal", "high"].includes(b.priority), "priority");
  if (b.labels !== undefined) valid(Array.isArray(b.labels) && b.labels.every((x: any) =>
    typeof x === "string" && x.trim().length <= 30 && !/\s/.test(x.trim())), "labels");
  if (b.assigneeId !== undefined) valid(b.assigneeId === null ||
    c.s.members.get(t.projectId)!.has(b.assigneeId), "assigneeId");
  for (const f of fields) if (b[f] !== undefined) {
    const v = f === "title" ? b[f].trim() : f === "labels"
      ? [...new Set(b[f].map((x: string) => x.trim().toLowerCase()))].sort() : b[f];
    if (JSON.stringify(t[f as keyof Task]) !== JSON.stringify(v)) {
      changed(c, t, user, f, t[f as keyof Task], v);
      (t as any)[f] = v;
    }
  }
  if (JSON.stringify(original) !== JSON.stringify(t)) { t.version++; t.updatedAt = iso(c.s); }
  return t;
}
