import type { State } from "./state";
import { bad, body, err, iso, record, taskReply } from "./util";
import { permission, validateTask } from "./task-core";

export async function update(req: Request, s: State, u: any, t: any, p: any) {
  if (!["owner", "member"].includes(permission(p, u))) return err(403, "forbidden", "Cannot update");
  const b = await body(req); if (!b) return err(400, "validation", "Invalid JSON");
  for (const key of Object.keys(b)) if (!["title", "description", "priority", "labels", "assigneeId"].includes(key))
    return bad(key);
  const field = validateTask({ ...t, ...b }, s, p); if (field) return bad(field);
  const match = req.headers.get("If-Match");
  if (match && match !== `"${t.version}"`) return err(412, "precondition_failed", "Version changed");
  const fields = Object.keys(b).sort(), changes: any[] = [];
  for (const key of fields) {
    const value = key === "title" ? b[key].trim() : key === "labels" ?
      [...new Set(b[key].map((x: string) => x.trim().toLowerCase()))].sort() : b[key];
    const old = t[key];
    if (JSON.stringify(old) !== JSON.stringify(value)) {
      changes.push([key, old, value]); t[key] = value;
    }
  }
  if (changes.length) {
    t.version++; t.updatedAt = iso(s);
    for (const [key, from, to] of changes) record(s, t, u.id, key, from, to);
  }
  return taskReply(t);
}

export async function transition(req: Request, s: State, u: any, t: any) {
  const p = s.projects.find(x => x.id === t.projectId);
  if (!["owner", "member"].includes(permission(p, u))) return err(403, "forbidden", "Cannot transition");
  const b = await body(req); if (!b) return err(400, "validation", "Invalid JSON");
  if (!["open", "in_progress", "done"].includes(b.to)) return bad("to");
  const match = req.headers.get("If-Match");
  if (match && match !== `"${t.version}"`) return err(412, "precondition_failed", "Version changed");
  const allowed: Record<string, string[]> = { open: ["in_progress"], in_progress: ["open", "done"], done: ["open"] };
  if (!allowed[t.status]?.includes(b.to)) return err(409, "invalid_transition", "Invalid transition");
  if (b.to === "done") {
    const blocker = [...s.dependencies].filter(x => x.startsWith(`${t.id}:`))
      .some(x => s.tasks.find(v => v.id === Number(x.split(":")[1]))?.status !== "done");
    if (blocker) return err(409, "blocked", "Task has unfinished dependencies");
  }
  const from = t.status; t.status = b.to; t.version++; t.updatedAt = iso(s);
  record(s, t, u.id, "status", from, b.to);
  return taskReply(t);
}
