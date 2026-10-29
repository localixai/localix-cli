import { State, Task, bad, byTaskKey, error, iso, json, memberRole, projectForTask } from "./model";
import { createTask, labels, validateTask, writeHistory } from "./task-core";
import { extraTask } from "./task-extra";
import { listTasks } from "./task-list";
export function taskRoutes(s: State, method: string, path: string, url: URL, body: any, uid: number, headers: Headers) {
  const pm = path.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if (pm) {
    const p = s.projects.find((x) => x.id === Number(pm[1]));
    if (!p || !memberRole(s, p, uid)) return error("not_found", "Project not found", 404);
    if (!pm[2] && method === "GET") return listTasks(s, p, url, uid);
    if (method === "POST") {
      if (memberRole(s, p, uid) === "viewer") return error("forbidden", "Forbidden", 403);
      const bulk = pm[2] === "bulk";
      const items = bulk ? body.tasks : [body];
      if (bulk && (!Array.isArray(items) || items.length < 1 || items.length > 50)) return bad("tasks");
      if (bulk) for (let i=0;i<items.length;i++) {
        const field = validateTask(s,p,items[i]);
        if (field) return bad(`tasks[${i}].${field}`);
      }
      else { const field = validateTask(s,p,body); if (field) return bad(field); }
      const idem = headers.get("Idempotency-Key"), idemKey = idem && `${uid}:${idem}`;
      if (idemKey) {
        const saved = s.idem.get(idemKey);
        if (saved && s.now()-saved.at < 86400000) {
          if (saved.body !== JSON.stringify(body)) {
            return error("idempotency_mismatch", "Key used with another body", 422);
          }
          return json(saved.result, 201, { "Idempotent-Replayed": "true", ETag: `"${saved.result.version}"` });
        }
      }
      const result = bulk ? { items: items.map((item: any) => createTask(s,p,uid,item)) } : createTask(s,p,uid,body);
      if (idemKey && !bulk) s.idem.set(idemKey, { at: s.now(), body: JSON.stringify(body), result });
      return json(result, 201, bulk ? undefined : { ETag: '"1"' });
    }
  }
  const m = path.match(
    /^\/tasks\/([A-Z]+-\d+)(?:\/(comments|history|dependencies|transitions|dependencies\/([A-Z]+-\d+)))?$/);
  if (!m) return;
  const t = byTaskKey(s,m[1]!);
  if (!t) return error("not_found", "Task not found", 404);
  if (m[2] === "comments" || m[2] === "history" || m[2] === "dependencies" || m[2]?.startsWith("dependencies/")) {
    if (m[2]?.startsWith("dependencies/") && method === "DELETE") {
      const other = byTaskKey(s,m[3]!);
      if (!other || !s.deps.get(t.id)!.delete(other.id)) {
        return error("not_found", "Dependency not found", 404);
      }
      return new Response(null,{status:204});
    }
    return extraTask(s,method,path,body,uid,t);
  }
  const p = projectForTask(s,t), role = memberRole(s,p,uid);
  if (!role) return error("not_found", "Task not found", 404);
  if (method === "GET") return json(t,200,{ ETag: `"${t.version}"` });
  if (role === "viewer") return error("forbidden", "Forbidden", 403);
  const match = headers.get("If-Match");
  if (match && match !== `"${t.version}"`) return error("precondition_failed", "Task changed", 412);
  if (method === "DELETE") {
    if (uid !== p.ownerId && t.createdBy !== uid) return error("forbidden", "Forbidden", 403);
    s.tasks = s.tasks.filter((x) => x.id !== t.id); s.deps.delete(t.id);
    s.deps.forEach((set) => set.delete(t.id)); return new Response(null,{status:204});
  }
  if (method === "PATCH") {
    const allowed = ["title","description","priority","labels","assigneeId"];
    for (const key of Object.keys(body)) if (!allowed.includes(key)) return bad(key);
    const field = validateTask(s,p,{ ...t, ...body }); if (field) return bad(field);
    let changed = false;
    for (const key of allowed.sort()) if (key in body) {
      const next = key === "labels" ? labels(body[key]) : key === "title" ? body[key].trim() : body[key];
      const prev = (t as any)[key];
      if (JSON.stringify(prev) !== JSON.stringify(next)) {
        writeHistory(s,t,uid,key,prev,next);
        (t as any)[key]=next;
        changed=true;
      }
    }
    if (changed) { t.version++; t.updatedAt=iso(s); }
    return json(t,200,{ ETag: `"${t.version}"` });
  }
  if (m[2] === "transitions" && method === "POST") {
    if (!["open","in_progress","done"].includes(body.to)) return bad("to");
    const valid = (t.status === "open" && body.to === "in_progress") ||
      (t.status === "in_progress" && ["open","done"].includes(body.to)) || (t.status === "done" && body.to === "open");
    if (!valid) return error("invalid_transition", "Invalid transition", 409);
    if (body.to === "done" && [...s.deps.get(t.id)!].some((id) => s.tasks.find((x) => x.id === id)?.status !== "done"))
      return error("blocked", "Blocked by dependencies", 409);
    writeHistory(s,t,uid,"status",t.status,body.to); t.status=body.to; t.version++; t.updatedAt=iso(s);
    return json(t,200,{ ETag: `"${t.version}"` });
  }
}
