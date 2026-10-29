import type { RequestCtx } from "./http";
import { bad, body, conflict, err, ok } from "./http";
import { iso, memberRole, pair, projectOf } from "./model";
import { normalizeLabels } from "./task-common";

const validate = (b: Record<string, any>, c: RequestCtx, projectId: number, prefix = "") => {
  if (typeof b.title !== "string" || !b.title.trim() || b.title.trim().length > 200)
    return bad(`${prefix}title`);
  if (b.description !== undefined && typeof b.description !== "string") return bad(`${prefix}description`);
  if (b.priority !== undefined && !["low", "normal", "high"].includes(b.priority)) return bad(`${prefix}priority`);
  if (b.labels !== undefined && normalizeLabels(b.labels) === null) return bad(`${prefix}labels`);
  if (b.assigneeId !== undefined && b.assigneeId !== null &&
    !memberRole(c.state, projectId, b.assigneeId)) return bad(`${prefix}assigneeId`);
};
function make(b: Record<string, any>, c: RequestCtx, p: NonNullable<ReturnType<typeof projectOf>>) {
  const number = (c.state.sequences.get(p.id) ?? 0) + 1;
  c.state.sequences.set(p.id, number);
  const now = iso(c);
  const t = { id: ++c.state.ids.task, key: `${p.key}-${number}`, number, projectId: p.id,
    title: b.title.trim(), description: b.description ?? "", status: "open",
    priority: b.priority ?? "normal", labels: normalizeLabels(b.labels ?? [])!,
    assigneeId: b.assigneeId ?? null, createdBy: c.userId, createdAt: now, updatedAt: now, version: 1 };
  c.state.tasks.push(t); return t;
}
export async function createTasks(path: string, c: RequestCtx) {
  const m = path.match(/^\/projects\/(\d+)\/tasks(\/bulk)?$/); if (!m) return undefined;
  const id = Number(m[1]), p = projectOf(c.state,id);
  if (!p || !memberRole(c.state,id,c.userId)) return err(404,"not_found","Not found");
  if (memberRole(c.state,id,c.userId) === "viewer") return err(403,"forbidden","Forbidden");
  const b = await body(c.req); if (!b) return bad("body");
  if (!m[2]) {
    const invalid = validate(b,c,id); if (invalid) return invalid;
    const key = c.req.headers.get("Idempotency-Key");
    const ik = key ? `${c.userId}:${key}` : "";
    const old = ik ? c.state.idem.get(ik) : undefined;
    if (old && c.now() - old.at < 86400000) {
      if (JSON.stringify(old.input) !== JSON.stringify(b))
        return err(422,"idempotency_mismatch","Idempotency key reused");
      return ok(old.task,201,{ ETag: '"1"', "Idempotent-Replayed": "true" });
    }
    const t = make(b,c,p);
    if (ik) c.state.idem.set(ik,{ at:c.now(), input:b, task:t });
    return ok(t,201,{ ETag: '"1"' });
  }
  if (!Array.isArray(b.tasks) || b.tasks.length < 1 || b.tasks.length > 50) return bad("tasks");
  for (let i=0;i<b.tasks.length;i++) {
    if (!b.tasks[i] || typeof b.tasks[i] !== "object") return bad(`tasks[${i}]`);
    const invalid = validate(b.tasks[i],c,id,`tasks[${i}].`); if (invalid) return invalid;
  }
  return ok({ items: b.tasks.map((x: Record<string,any>) => make(x,c,p)) },201);
}
